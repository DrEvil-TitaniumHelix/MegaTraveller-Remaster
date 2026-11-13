use super::ui;
use crate::ground::{render, state::Ground};
use mt_engine::data::{dict, msg, Variant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dialogue { pub lines: Vec<Vec<u8>>, pub short: Option<Vec<u8>> }

const MAX_FILE_BYTES: usize = 1024 * 1024;
const MAX_DICTIONARY_BYTES: usize = 65536;
const MAX_DICTIONARY_WORDS: usize = 32768;
const MAX_WORD_BYTES: usize = 1024;
const MAX_TOKENS: usize = 65535;
const MAX_EXPANDED_BYTES: usize = 65536;
const MAX_LINES: usize = 4096;

fn check_dictionary<'a>(words: impl IntoIterator<Item = &'a [u8]>) -> Result<(), String> {
    let mut bytes = 0usize;
    for (i, word) in words.into_iter().enumerate() {
        if i >= MAX_DICTIONARY_WORDS || word.len() > MAX_WORD_BYTES { return Err("dialogue: dictionary budget exceeded".into()) }
        bytes += word.len();
        if bytes > MAX_DICTIONARY_BYTES { return Err("dialogue: dictionary byte budget exceeded".into()) }
    }
    Ok(())
}

pub fn decode_dialogue(tokens: &[i16], dictionary: &[Vec<u8>], width: i16, index: i16, mode: i16) -> Result<Dialogue, String> {
    if tokens.len() > MAX_TOKENS { return Err("dialogue: token budget exceeded".into()) }
    check_dictionary(dictionary.iter().map(Vec::as_slice))?;
    let tokens = if mode != 0 && tokens.len() > 7 { &tokens[..6] } else { tokens };
    let mut line = if index >= 5 { vec![b'"'] } else { vec![] }; let mut lines = vec![];
    let mut expanded = if index >= 5 { 2 } else { 0 } + if mode == 1 { 3 } else { 0 };
    for (i, &token) in tokens.iter().enumerate() {
        let literal = [token.wrapping_neg() as u8, b' '];
        let (word, space): (&[u8], bool) = if token == -1 { (b"<invalid> ", false) }
        else if token < 0 { (&literal, false) }
        else {
            let Some(word) = dictionary.get(token as usize) else { return Err(format!("dialogue: invalid dictionary index {token}")) };
            (word, i != 0)
        };
        let len = word.len() + usize::from(space);
        let wraps = line.len() as i64 + len as i64 > (width / 6) as i64;
        if wraps && mode == 1 { break }
        if len > MAX_EXPANDED_BYTES - expanded { return Err("dialogue: expanded byte budget exceeded".into()) }
        expanded += len;
        if wraps {
            if lines.len() >= MAX_LINES - 1 { return Err("dialogue: line budget exceeded".into()) }
            lines.push(std::mem::take(&mut line));
        }
        if space { line.push(b' ') }
        line.extend_from_slice(word);
    }
    if mode == 1 { line.extend_from_slice(b"...") }
    if index >= 5 { line.push(b'"') }
    let short = if mode != 0 { Some(line) } else { lines.push(line); None };
    Ok(Dialogue { lines, short })
}

pub fn set_text_box(g: &mut Ground, mode: i16) {
    g.mem.set_i16(0x6E0A, mode);
    let values = match mode { 1 => [5,6,309,164,21], 2 => [57,0,192,141,15], 3 => [169,1,189,40,169], _ => [6,162,152,164,18] };
    for (off, v) in [0x6E0E,0x6E08,0x6E00,0x6E06,0x6E04].into_iter().zip(values) { g.mem.set_i16(off, v) }
}

pub fn get_message(g: &mut Ground, index: i16, reset: bool, mode: i16) -> Result<Option<Vec<u8>>, String> {
    g.enter("get_message");
    let mut read = |name: &str| g.files().read_limited(name, MAX_FILE_BYTES).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound { format!("unable to open file {}", name.to_ascii_lowercase()) }
        else { format!("{name}: {e}") }
    });
    let messages = read("TRAV.MSG")?;
    let dictionary = read("TRAV.DAT")?;
    let messages = msg::decode(Variant::Mt1, &messages).map_err(|e| format!("TRAV.MSG: {e:?}"))?;
    let end = dictionary.iter().position(|&b| b == b'@').ok_or("TRAV.DAT: missing terminator")?;
    if end == 0 || dictionary[end - 1] != b'^' { return Err("TRAV.DAT: unterminated word".into()) }
    check_dictionary(dictionary[..end - 1].split(|&b| b == b'^'))?;
    let dictionary = dict::decode(&dictionary).map_err(|e| format!("TRAV.DAT: {e:?}"))?;
    let tokens = messages.msgs.get(index as usize).ok_or_else(|| format!("dialogue: invalid message index {index}"))?;
    let d = decode_dialogue(tokens, &dictionary.words, g.mem.i16(0x6E00), index, mode)?;
    g.enter("print_message");
    if reset { g.mem.set_u8(0x6E2A, 0) }
    g.mem.set_u8(0x713F, g.mem.u8(0x713F) | 1);
    for line in d.lines {
        let y = g.mem.u8(0x6E2A) as i32 * 8 + g.mem.i16(0x6E0E) as i32;
        let x = g.mem.i16(0x6E08) as i32 + 8;
        ui::text(g, &line, y, x); g.mem.set_u8(0x6E2A, g.mem.u8(0x6E2A).wrapping_add(1));
    }
    ui::frame(g,"print_message");
    g.mem.set_u8(0x713F, g.mem.u8(0x713F) & !1);
    ui::frame(g,"get_message");
    Ok(d.short)
}

pub fn say_message(g: &mut Ground, index: i16, timed: bool) -> Result<(), String> {
    g.enter("say_message"); ui::ink(g, 6);
    if !timed { ui::target(g, true); ui::layout(g, 1, 54, 0) }
    else { render::dismiss_message(g, 0) }
    let result = get_message(g, index, true, 0);
    if !timed { ui::target(g, false) }
    else { ui::present(g, 169, 0, 31, 24); let d = g.now().wrapping_add(180); g.mem.set_message_deadline(d) }
    if result.is_ok() { ui::frame(g,"say_message") }
    result.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ground::state::Mem;
    use mt_engine::{io::{Files, Overlay, Scripted}, sim::tick::Counter};

    #[test]
    fn dialogue_expansion_is_bounded_before_copying_words() {
        let words = vec![vec![b'x'; 1023], vec![]];
        let mut tokens = vec![0; 64];
        let size = |d: Dialogue| d.lines.iter().map(Vec::len).sum::<usize>();
        assert_eq!(size(decode_dialogue(&tokens,&words,i16::MAX,0,0).unwrap()),MAX_EXPANDED_BYTES-1);
        tokens.push(1);
        assert_eq!(size(decode_dialogue(&tokens,&words,i16::MAX,0,0).unwrap()),MAX_EXPANDED_BYTES);
        assert!(decode_dialogue(&tokens,&words,i16::MAX,5,0).unwrap_err().contains("expanded"));
        tokens.push(-65);
        assert!(decode_dialogue(&tokens,&words,i16::MAX,0,0).unwrap_err().contains("expanded"));
        assert!(decode_dialogue(&vec![0;MAX_TOKENS],&words,189,5,1).is_ok());
        assert!(decode_dialogue(&vec![0;MAX_TOKENS],&words,189,5,2).is_ok());
        assert!(decode_dialogue(&vec![0;MAX_TOKENS],&words,189,5,0).unwrap_err().contains("expanded"));
    }

    #[test]
    fn dialogue_dictionary_tokens_and_empty_lines_have_budgets() {
        for words in [vec![vec![b'x';MAX_WORD_BYTES+1]], vec![vec![];MAX_DICTIONARY_WORDS+1],
            vec![vec![b'x';MAX_WORD_BYTES];MAX_DICTIONARY_BYTES/MAX_WORD_BYTES+1]] {
            assert!(decode_dialogue(&[],&words,189,0,0).unwrap_err().contains("dictionary"));
        }
        assert!(decode_dialogue(&[],&vec![vec![];MAX_DICTIONARY_WORDS],189,0,0).is_ok());
        assert!(decode_dialogue(&vec![0;MAX_TOKENS+1],&[vec![]],189,0,1).unwrap_err().contains("token"));
        assert_eq!(decode_dialogue(&vec![0;MAX_LINES-1],&[vec![]],-6,0,0).unwrap().lines.len(),MAX_LINES);
        assert!(decode_dialogue(&vec![0;MAX_LINES],&[vec![]],-6,0,0).unwrap_err().contains("line"));
    }

    fn message_host(messages: &[u8], dictionary: &[u8]) -> Ground {
        let mut files = Overlay::new(None);
        files.write("TRAV.MSG",messages).unwrap(); files.write("TRAV.DAT",dictionary).unwrap();
        Ground::from_ds(vec![0;65536],vec![0;Mem::PARTY_SIZE],Box::new(Scripted::new([])),Box::new(Counter::default()),Box::new(files))
    }

    #[test]
    fn dialogue_rejects_amplification_pair_without_drawing_or_mutating_state() {
        let messages = msg::encode(Variant::Mt1,&msg::Msgs { msgs:vec![vec![0;8192]],tail:vec![] }).unwrap();
        let dictionary = dict::encode(&dict::Dict { words:vec![vec![b'x';8192]],tail:vec![] });
        assert_eq!(messages.len()+dictionary.len(),24586);
        for word_len in [8192,1024] {
            let dictionary = dict::encode(&dict::Dict { words:vec![vec![b'x';word_len]],tail:vec![] });
            let mut g = message_host(&messages,&dictionary); g.harness = true;
            g.mem.set_u8(0x6E2A,17); g.mem.set_i16(0x6E00,189);
            let before = g.mem.ds.clone();
            let error = get_message(&mut g,0,true,0).unwrap_err();
            assert!(error.contains(if word_len == 8192 { "dictionary" } else { "expanded" }),"{error}");
            assert_eq!(g.mem.ds,before); assert!(g.log.is_empty());
        }
    }

    #[test]
    fn dialogue_rejects_oversized_files_before_decode() {
        let messages = msg::encode(Variant::Mt1,&msg::Msgs { msgs:vec![vec![0]],tail:vec![] }).unwrap();
        for (name, m, d) in [("TRAV.MSG",vec![0;MAX_FILE_BYTES+1],b"word^@".to_vec()),
            ("TRAV.DAT",messages,vec![0;MAX_FILE_BYTES+1])] {
            let mut g = message_host(&m,&d);
            let error = get_message(&mut g,0,true,0).unwrap_err();
            assert!(error.contains(name) && error.contains("file byte budget"),"{error}");
        }
    }
}
