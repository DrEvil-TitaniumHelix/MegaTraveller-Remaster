use super::*;
use mt_engine::{io::Event, play::{Charset, Decision, Fnv, Kind, Option, TextInput, HASH_VERSION}};
impl Screens {
    pub fn play_hash(&mut self) -> String {
        let mut h = Fnv::default(); h.write(&HASH_VERSION.to_le_bytes()); h.field(b"cg");
        h.field(&self.ds.image()); h.field(&self.work); h.write(&self.rng.state.to_le_bytes());
        h.write(&self.clock.now().to_le_bytes()); h.write(&self.clock.ticks().to_le_bytes());
        h.field(format!("{:?}",self.pool_record).as_bytes());
        if let Some(o) = &self.overlay { for (name, data) in o.written() { h.field(name.as_bytes()); h.field(data); } }
        h.field(&self.files().read("UCHARS.DAT").unwrap_or_default());
        h.field(format!("{:?}",self.wins).as_bytes());
        for v in [self.initial,self.msg_delay,self.msg_win,self.skills_win,self.skills_title,self.help_w,self.help_left,
            self.prev_total,self.prev_limit,self.color,self.row,self.col,self.row_off,self.col_off,self.shadow,self.centre,self.line_h,self.spacing] { h.write(&v.to_le_bytes()); }
        h.write(&[self.input_changed as u8,self.flags,self.pen.colour,self.pen.op]);
        h.write(&self.text_speed.to_le_bytes()); h.write(&self.timer.start.to_le_bytes());
        h.write(&(self.cur_win as u64).to_le_bytes());
        h.write(&self.menu_mouse.0.to_le_bytes()); h.write(&self.menu_mouse.1.to_le_bytes());
        for (i,(key,_,enabled)) in self.bindings.iter().enumerate() { h.write(&(i as u64).to_le_bytes()); h.write(&key.to_le_bytes()); h.write(&[*enabled as u8]); }
        h.field(&self.fb.regs); for c in self.dac { h.write(&c); }
        h.state()
    }
    pub(super) fn play_decide(&mut self, mut d: Decision) {
        if self.lockstep.is_some() || !self.input.wants_decision() { return }
        if self.input.data_mode() {
            let mut state = crate::play::observation::Observation::program(crate::ui::ScreenKind::Cg, self.clock.now());
            state.draft = Some(self.work.to_vec()); self.input.scene(&state);
        }
        d.state = self.play_hash(); self.input.decide(&d);
    }
    pub(super) fn play_menu(&mut self, m: &Menu, selected: i16, horizontal: bool) {
        if self.lockstep.is_some() || !self.input.wants_decision() { return }
        let routine = if self.play_yesno { "1AA2" } else if horizontal { "56A6" } else { "519A" };
        let mut d = Decision::new(if self.play_yesno { Kind::YesNo } else { Kind::Menu }, "cg", routine);
        d.context.table = Some(format!("{},{}",m.row,m.col));
        for (i,label) in m.items.iter().enumerate() {
            let key = if horizontal { if i as i16 >= selected {keys::RIGHT} else {keys::LEFT} } else if i as i16 >= selected {keys::DOWN} else {keys::UP};
            let mut input = vec![Event::Key(key); (i as i16).abs_diff(selected) as usize]; input.push(Event::Key(keys::ENTER));
            let id = if self.play_yesno { if i==0 {"yes".into()} else {"no".into()} } else {format!("menu:{}.{}:{}/{i}",routine.to_ascii_lowercase(),m.row,m.col)};
            if i as i16 == selected { d.default = Some(id.clone()); }
            d.options.push(Option::new(id,*label,input));
        }
        if !self.play_yesno { d.options.push(Option::new("back","Cancel",vec![Event::Key(keys::ESC)])); }
        self.play_decide(d);
    }
    pub(super) fn play_text(&mut self, len: usize, width: i16) {
        if self.lockstep.is_some() || !self.input.wants_decision() { return }
        let mut d = Decision::new(Kind::Text,"cg","5CC2");
        d.input = Some(TextInput { max_len: len, charset: Charset::Ascii((32..=126).map(char::from).collect()),
            width: Some((width.saturating_sub(4).max(0) as u32,self.font.widths.to_vec())), submit: vec![Event::Key(keys::ENTER)] });
        d.options.push(Option::new("back","Cancel",vec![Event::Key(keys::ESC)])); self.play_decide(d);
    }
    pub(super) fn play_ack(&mut self) {
        self.play_decide(crate::play_prompts::keys_decision("cg","7290",Kind::Ack,&[("continue","Continue",32)]));
    }
}
