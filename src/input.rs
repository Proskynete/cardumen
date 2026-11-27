//! Key events to the bytes an xterm-compatible program expects.
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// xterm modifier parameter: 1 + shift + 2·alt + 4·ctrl.
fn modifier_param(m: ModifiersState) -> u8 {
    1 + m.shift_key() as u8 + 2 * m.alt_key() as u8 + 4 * m.control_key() as u8
}

fn ctrl_byte(c: char) -> Option<u8> {
    match c.to_ascii_lowercase() {
        c @ 'a'..='z' => Some(c as u8 - b'a' + 1),
        '@' | ' ' | '2' => Some(0),
        '[' | '3' => Some(0x1b),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '-' | '7' => Some(0x1f),
        '?' | '8' => Some(0x7f),
        _ => None,
    }
}

/// `key` is the logical key; `plain` is the same key without modifiers applied
/// (on macOS, Option turns `b` into `∫`, and we want Meta-b instead).
pub fn encode(key: &Key, plain: &Key, text: Option<&str>, mods: ModifiersState, app_cursor: bool) -> Option<Vec<u8>> {
    let esc = |mut v: Vec<u8>| {
        if mods.alt_key() {
            v.insert(0, 0x1b);
        }
        v
    };
    match key {
        Key::Named(named) => encode_named(*named, mods, app_cursor),
        Key::Character(_) if mods.super_key() => None,
        Key::Character(s) if mods.control_key() => {
            let c = match plain {
                Key::Character(p) => p.chars().next(),
                _ => s.chars().next(),
            }?;
            ctrl_byte(c).map(|b| esc(vec![b]))
        }
        Key::Character(s) if mods.alt_key() => match plain {
            Key::Character(p) => {
                let p = if mods.shift_key() { p.to_uppercase() } else { p.to_string() };
                Some(esc(p.into_bytes()))
            }
            _ => Some(s.as_bytes().to_vec()),
        },
        _ => text.map(|t| t.as_bytes().to_vec()),
    }
}

fn encode_named(key: NamedKey, mods: ModifiersState, app_cursor: bool) -> Option<Vec<u8>> {
    let m = modifier_param(mods);
    let alt_only = mods.alt_key() && !mods.shift_key() && !mods.control_key() && !mods.super_key();
    let csi_letter = |c: char| -> Vec<u8> {
        if m > 1 {
            format!("\x1b[1;{m}{c}").into_bytes()
        } else if app_cursor {
            format!("\x1bO{c}").into_bytes()
        } else {
            format!("\x1b[{c}").into_bytes()
        }
    };
    let tilde = |n: u8| -> Vec<u8> {
        if m > 1 { format!("\x1b[{n};{m}~").into_bytes() } else { format!("\x1b[{n}~").into_bytes() }
    };
    let ss3 = |c: char| -> Vec<u8> {
        if m > 1 { format!("\x1b[1;{m}{c}").into_bytes() } else { format!("\x1bO{c}").into_bytes() }
    };
    let alt = |b: &[u8]| -> Vec<u8> {
        let mut v = b.to_vec();
        if mods.alt_key() {
            v.insert(0, 0x1b);
        }
        v
    };
    // macOS line-editing conventions: Cmd moves by line, Option by word.
    if mods.super_key() {
        return match key {
            NamedKey::ArrowLeft => Some(vec![0x01]),
            NamedKey::ArrowRight => Some(vec![0x05]),
            NamedKey::Backspace => Some(vec![0x15]),
            _ => None,
        };
    }
    Some(match key {
        NamedKey::ArrowLeft if alt_only => b"\x1bb".to_vec(),
        NamedKey::ArrowRight if alt_only => b"\x1bf".to_vec(),
        NamedKey::ArrowUp => csi_letter('A'),
        NamedKey::ArrowDown => csi_letter('B'),
        NamedKey::ArrowRight => csi_letter('C'),
        NamedKey::ArrowLeft => csi_letter('D'),
        NamedKey::Home => csi_letter('H'),
        NamedKey::End => csi_letter('F'),
        NamedKey::Insert => tilde(2),
        NamedKey::Delete => tilde(3),
        NamedKey::PageUp => tilde(5),
        NamedKey::PageDown => tilde(6),
        NamedKey::F1 => ss3('P'),
        NamedKey::F2 => ss3('Q'),
        NamedKey::F3 => ss3('R'),
        NamedKey::F4 => ss3('S'),
        NamedKey::F5 => tilde(15),
        NamedKey::F6 => tilde(17),
        NamedKey::F7 => tilde(18),
        NamedKey::F8 => tilde(19),
        NamedKey::F9 => tilde(20),
        NamedKey::F10 => tilde(21),
        NamedKey::F11 => tilde(23),
        NamedKey::F12 => tilde(24),
        // Shift+Enter as Meta-Enter: newline in Claude Code's prompt and zsh (self-insert-unmeta).
        NamedKey::Enter if mods.shift_key() => b"\x1b\r".to_vec(),
        NamedKey::Enter => alt(b"\r"),
        NamedKey::Tab if mods.shift_key() => b"\x1b[Z".to_vec(),
        NamedKey::Tab => alt(b"\t"),
        NamedKey::Backspace if mods.control_key() => vec![0x08],
        NamedKey::Backspace => alt(&[0x7f]),
        NamedKey::Escape => vec![0x1b],
        NamedKey::Space if mods.control_key() => vec![0],
        NamedKey::Space => alt(b" "),
        _ => return None,
    })
}

/// Backslash-escapes a path for the shell, like Terminal.app does on drop.
pub fn shell_escape(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        if !(c.is_alphanumeric() || "/._-+,:@%".contains(c)) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

pub enum Edit {
    Continue,
    Commit,
    Cancel,
}

/// Line editing for the tab-rename field.
pub fn edit_line(buf: &mut String, key: &Key, text: Option<&str>, mods: ModifiersState) -> Edit {
    match key {
        Key::Named(NamedKey::Enter) => Edit::Commit,
        Key::Named(NamedKey::Escape) => Edit::Cancel,
        Key::Named(NamedKey::Backspace) if mods.super_key() => {
            buf.clear();
            Edit::Continue
        }
        Key::Named(NamedKey::Backspace) => {
            buf.pop();
            Edit::Continue
        }
        _ if mods.super_key() || mods.control_key() => Edit::Continue,
        _ => {
            buf.extend(text.unwrap_or("").chars().filter(|c| !c.is_control()));
            Edit::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(s: &str) -> Key {
        Key::Character(s.into())
    }

    fn named(k: NamedKey, mods: ModifiersState, app: bool) -> Vec<u8> {
        encode(&Key::Named(k), &Key::Named(k), None, mods, app).unwrap()
    }

    #[test]
    fn plain_text() {
        assert_eq!(encode(&ch("a"), &ch("a"), Some("a"), ModifiersState::empty(), false).unwrap(), b"a");
    }

    #[test]
    fn ctrl_c_is_etx() {
        assert_eq!(encode(&ch("c"), &ch("c"), None, ModifiersState::CONTROL, false).unwrap(), [3]);
    }

    #[test]
    fn option_is_meta() {
        assert_eq!(encode(&ch("∫"), &ch("b"), Some("∫"), ModifiersState::ALT, false).unwrap(), b"\x1bb");
    }

    #[test]
    fn arrows() {
        assert_eq!(named(NamedKey::ArrowUp, ModifiersState::empty(), false), b"\x1b[A");
        assert_eq!(named(NamedKey::ArrowUp, ModifiersState::empty(), true), b"\x1bOA");
        assert_eq!(named(NamedKey::ArrowUp, ModifiersState::SHIFT, false), b"\x1b[1;2A");
        assert_eq!(named(NamedKey::ArrowLeft, ModifiersState::ALT, false), b"\x1bb");
        assert_eq!(named(NamedKey::ArrowLeft, ModifiersState::SUPER, false), [1]);
    }

    #[test]
    fn misc_keys() {
        assert_eq!(named(NamedKey::Enter, ModifiersState::empty(), false), b"\r");
        assert_eq!(named(NamedKey::Enter, ModifiersState::SHIFT, false), b"\x1b\r");
        assert_eq!(named(NamedKey::Backspace, ModifiersState::empty(), false), [0x7f]);
        assert_eq!(named(NamedKey::Tab, ModifiersState::SHIFT, false), b"\x1b[Z");
        assert_eq!(named(NamedKey::Delete, ModifiersState::CONTROL, false), b"\x1b[3;5~");
        assert_eq!(named(NamedKey::F5, ModifiersState::empty(), false), b"\x1b[15~");
    }

    #[test]
    fn escapes_dropped_paths() {
        assert_eq!(shell_escape("/Users/a/Captura de pantalla (1).png"), "/Users/a/Captura\\ de\\ pantalla\\ \\(1\\).png");
        assert_eq!(shell_escape("/tmp/añadido.png"), "/tmp/añadido.png");
    }

    #[test]
    fn rename_editing() {
        let mut b = String::from("ab");
        let none = ModifiersState::empty();
        assert!(matches!(edit_line(&mut b, &ch("c"), Some("c"), none), Edit::Continue));
        assert_eq!(b, "abc");
        edit_line(&mut b, &Key::Named(NamedKey::Backspace), None, none);
        assert_eq!(b, "ab");
        edit_line(&mut b, &ch("x"), Some("x"), ModifiersState::CONTROL);
        assert_eq!(b, "ab");
        edit_line(&mut b, &Key::Named(NamedKey::Backspace), None, ModifiersState::SUPER);
        assert_eq!(b, "");
        assert!(matches!(edit_line(&mut b, &Key::Named(NamedKey::Enter), None, none), Edit::Commit));
        assert!(matches!(edit_line(&mut b, &Key::Named(NamedKey::Escape), None, none), Edit::Cancel));
    }
}
