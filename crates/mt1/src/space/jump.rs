pub type Party = [u8; 0xF86];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub video_mode: i16,
    pub world_time: i16,
    pub system_encounter: i16,
    pub view_heading: i16,
    pub minimap_quadrant: i16,
    pub thrust: i16,
    pub heading: i16,
    pub moving: i16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label { pub y: i16, pub x: i16, pub text: Vec<u8>, pub flags: u8 }

#[derive(Clone, Debug)]
pub struct Tables {
    pub links: [u8; 24],
    pub danger: [i16; 8],
    pub names: [Vec<u8>; 8],
    pub rects: [[i16; 4]; 8],
    pub labels: Vec<Label>,
    pub system_for_row: [u8; 8],
    pub row_for_system: [u8; 8],
    ds: Vec<u8>,
}

impl Tables {
    pub fn from_exe(exe: &[u8]) -> Result<Self, String> {
        let header = word(exe, 8)? as u16 as usize * 16;
        let ds = exe.get(header + 0x161D0..).ok_or("jump: S image lacks DGROUP")?;
        Self::from_ds(ds)
    }
    pub fn from_ds(ds: &[u8]) -> Result<Self, String> {
        let mut t = Self { links: bytes(ds, 0x178)?, danger: [0; 8],
            names: std::array::from_fn(|_| Vec::new()), rects: [[0; 4]; 8], labels: Vec::new(),
            system_for_row: bytes(ds, 0x5CA)?, row_for_system: bytes(ds, 0x5D2)?, ds: ds.to_vec() };
        for i in 0..8 {
            t.danger[i] = word(ds, 0x3B1 + i * 2)?;
            t.names[i] = t.text(word(ds, 0x481 + i * 4)? as u16)?;
            for j in 0..4 { t.rects[i][j] = word(ds, 0x51E + i * 8 + j * 2)?; }
        }
        for i in 0..12 {
            let at = 0x55E + i * 9;
            let flags = *ds.get(at + 8).ok_or("jump: short labels")?;
            let off = word(ds, at + 4)? as u16;
            t.labels.push(Label { y: word(ds, at)?, x: word(ds, at + 2)?,
                text: if off == 0 { Vec::new() } else { t.text(off)? }, flags });
        }
        if t.system_for_row.iter().any(|&v| v >= 8) || t.row_for_system.iter().any(|&v| v >= 8) {
            return Err("jump: invalid system row table".into());
        }
        Ok(t)
    }
    pub(crate) fn text(&self, off: u16) -> Result<Vec<u8>, String> {
        let tail = self.ds.get(off as usize..).ok_or("jump: string outside DGROUP")?;
        let n = tail.iter().position(|&b| b == 0).ok_or("jump: unterminated string")?;
        Ok(tail[..n].to_vec())
    }
}
fn bytes<const N: usize>(ds: &[u8], at: usize) -> Result<[u8; N], String> {
    ds.get(at..at + N).and_then(|v| v.try_into().ok()).ok_or_else(|| "jump: truncated DGROUP".into())
}
fn word(ds: &[u8], at: usize) -> Result<i16, String> { Ok(i16::from_le_bytes(bytes(ds, at)?)) }

pub fn is_program_loaded(party: &Party, program: u16) -> bool {
    party[0xAC..0xC0].iter().any(|&p| p & 0x20 != 0 && u16::from(p & 15) == program)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    BackBuffer, CursorMode(i16), Viewport(i16, i16, i16, i16), Layout(i16, i16, i16),
    Labels(Vec<Label>), Color(i16), TextPosition(i16, i16), Text(Vec<u8>),
    Fill(i16, i16, i16, i16, i16), Flip,
    Select { rects: Vec<[i16; 4]>, initial: i16, highlight: i16, background: i16, keys: Vec<u8> },
    Rand, LoadSystem(u8), InitEncounters, FlightHud(i16), HideCursor, SystemView(i16),
    Thrust(i16, i16), UpdateMinimap, Copy(i16, i16, i16, i16),
    Restore(i16, i16, i16, i16), Animate, EncounterIcons(i16), Ticks, Palette(i16, i16),
}

pub trait Host {
    fn call(&mut self, call: Call, state: &mut State, party: &mut Party) -> Result<i32, String>;
    fn wait_iteration(&mut self) {}
    fn cancelled_row(&mut self, tables: &Tables) -> Result<u8, String> { Ok(tables.row_for_system[7]) }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    pub drive: u8, pub program: u8, pub error: Option<u16>, pub reachable: u8,
    pub rows: Vec<u8>, pub labels: Vec<Label>,
}

pub fn menu(party: &Party, tables: &Tables) -> Result<Menu, String> {
    let system = party[6] as usize;
    if system >= 8 { return Err("jump: system outside 0..7".into()); }
    let mut error = (party[0xFC] <= 86).then_some(0x1334);
    let mut drive = party[0xC1] & 3;
    if drive == 0 { error = Some(0x1346); } else { drive -= 1; }
    let program = if is_program_loaded(party, 3) { 2 } else if is_program_loaded(party, 9) { 1 }
        else { error = Some(0x1354); 0 };
    if !is_program_loaded(party, 4) { error = Some(0x136C); }
    if drive == 1 && program != 2 { drive = 0; }
    let reachable = tables.links[drive as usize * 8 + system];
    let mut labels = tables.labels.clone();
    let mut rows = Vec::new();
    for (i, &s) in tables.system_for_row.iter().enumerate() {
        if reachable & (1 << s) != 0 {
            rows.push(i as u8);
            if let Some(label) = labels.get_mut(i) { label.flags = 6; }
        }
    }
    if rows.is_empty() { return Err("jump: empty reachable menu outside oracle contract".into()); }
    if let Some(label) = labels.get_mut(tables.row_for_system[system] as usize) { label.flags = 4; }
    Ok(Menu { drive, program, error, reachable, rows, labels })
}

pub fn jump_screen(host: &mut impl Host, state: &mut State, party: &mut Party, tables: &Tables) -> Result<(), String> {
    let m = menu(party, tables)?;
    macro_rules! call { ($c:expr) => { host.call($c, state, party)? }; }
    macro_rules! text { ($off:expr) => { call!(Call::Text(tables.text($off)?)) }; }
    call!(Call::BackBuffer); call!(Call::CursorMode(0)); call!(Call::Viewport(0, 0, 200, 40));
    call!(Call::Layout(20, 0, 0)); call!(Call::Labels(m.labels.clone()));
    call!(Call::Color(0)); call!(Call::TextPosition(174, 137));
    if m.program != 0 { text!(if m.program == 1 { 0x1383 } else { 0x1398 }); }
    if party[0xFC] != 0 { call!(Call::Fill(183, 4, 5, party[0xFC] as i16, 9)); }
    call!(Call::TextPosition(2, 4)); call!(Call::Text(tables.names[party[6] as usize].clone())); text!(0x13AD);
    let mut misjump = false;
    call!(Call::TextPosition(181, 137));
    if let Some(error) = m.error {
        text!(0x13BA); call!(Call::Color(if state.video_mode == 2 { 0 } else { 3 }));
        call!(Call::TextPosition(187, 137)); text!(error);
    } else if (87..=102).contains(&party[0xFC]) {
        misjump = call!(Call::Rand) as u16 % 100 < 30; text!(0x13CC);
    } else {
        let mut message = tables.text(0x13E9)?;
        *message.get_mut(5).ok_or("jump: short availability message")? = m.drive.wrapping_add(177);
        call!(Call::Text(message));
    }
    call!(Call::Flip);
    let mut choice = call!(Call::Select { rects: m.rows.iter().map(|&r| tables.rects[r as usize]).collect(),
        initial: 0, highlight: 6, background: 9, keys: tables.text(0x13FA)? }) as i16;
    if misjump {
        let index = (call!(Call::Rand) as u16 % m.rows.len() as u16) as i16;
        if index == choice { misjump = false; }
        choice = index;
    }
    let row = if choice == -1 { host.cancelled_row(tables)? } else {
        *m.rows.get(choice as usize).ok_or("jump: selection outside reachable menu")?
    };
    let destination = *tables.system_for_row.get(row as usize).ok_or("jump: invalid adjacent cancellation row")?;
    if m.error.is_none() && choice != -1 && destination != party[6] {
        party[6] = destination; party[7] = 255;
        let clock = i16::from_le_bytes([party[0xF84], party[0xF85]]).wrapping_add(7);
        party[0xF84..0xF86].copy_from_slice(&clock.to_le_bytes()); state.world_time = clock;
        state.system_encounter = tables.danger[destination as usize].wrapping_sub((call!(Call::Rand) as u16 % 2) as i16);
        call!(Call::LoadSystem(destination)); call!(Call::InitEncounters); party[0xFC] /= 2;
    } else if destination == party[6] { party[0xFC] /= 2; }
    call!(Call::Layout(21, 0, 0)); call!(Call::FlightHud(-1)); call!(Call::HideCursor);
    state.view_heading = -1; state.minimap_quadrant = -1;
    if call!(Call::SystemView(1)) != 0 { call!(Call::Thrust(state.thrust, 7)); }
    call!(Call::Layout(state.heading, 100, 128));
    if state.moving != 0 { call!(Call::Layout(state.heading.wrapping_add(120), 100, 128)); }
    call!(Call::UpdateMinimap); call!(Call::TextPosition(174, 137));
    if misjump { call!(Call::Color(3)); text!(0x1403); }
    else if choice != -1 && m.error.is_none() {
        call!(Call::Flip); call!(Call::Copy(0, 0, 200, 40)); call!(Call::Animate);
        call!(Call::TextPosition(174, 137)); call!(Call::Color(0)); text!(0x141D);
        state.view_heading = 0; call!(Call::Thrust(state.thrust, 9)); call!(Call::Restore(0, 0, 200, 40));
        return Ok(());
    }
    call!(Call::Flip); state.view_heading = 0; call!(Call::Thrust(state.thrust, 9)); call!(Call::Copy(0, 0, 200, 40));
    Ok(())
}

pub fn animate_jump_transition(host: &mut impl Host, state: &mut State, party: &mut Party) -> Result<(), String> {
    host.call(Call::BackBuffer, state, party)?;
    host.call(Call::EncounterIcons(0), state, party)?;
    state.view_heading = state.heading.wrapping_sub(90);
    for frame in 0..100 {
        let deadline = (host.call(Call::Ticks, state, party)? as u32).wrapping_add(frame / 60);
        host.call(Call::SystemView(1), state, party)?;
        while host.call(Call::Ticks, state, party)? as u32 <= deadline {
            host.wait_iteration();
            if frame > 20 && frame < 40 {
                host.call(Call::Palette(0, 4), state, party)?;
                host.call(Call::Palette(0, 0), state, party)?;
            }
        }
        host.call(Call::Flip, state, party)?;
    }
    Ok(())
}
