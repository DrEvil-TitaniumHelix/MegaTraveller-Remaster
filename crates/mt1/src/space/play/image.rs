use crate::{play::anchors::codec::Value, space::{self, Space}};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub ds:Vec<u8>, pub party:Vec<u8>, pub world:Vec<u8>,
    pub ticks:u32, pub now:u32, pub last_ticks:u32, pub dead:bool, pub scenario_entry:Option<String>,
    pub mouse:space::mouse::Driver, pub planes:Vec<u8>, pub regs:[u8;16], pub dac:mt_engine::present::palette::Dac,
    pub pen:mt_engine::present::ly::Pen, pub flight:space::flight::Flight,
    pub sounds:space::sounds::Image,
    pub text:Option<mt_engine::present::textlog::SurfaceImage>,
}
impl Image {
    fn capture(sp:&mut Space,flight:space::flight::Flight)->Self {
        Self {ds:sp.s.image().to_vec(),party:sp.s.party.clone(),world:sp.s.world.clone(),ticks:sp.clock.ticks(),now:sp.clock.now(),last_ticks:sp.last_ticks,
            dead:sp.dead,scenario_entry:sp.scenario_entry.clone(),mouse:sp.mouse,planes:sp.r.fb.planes.clone(),regs:sp.r.fb.regs,dac:sp.dac,pen:sp.r.pen,flight,sounds:sp.sounds.capture(),text:mt_engine::present::textlog::capture(&sp.r.fb)}
    }
    pub fn install(&self,sp:&mut Space)->Result<(),String> {
        if self.ds.len()!=65536 || self.party.len()!=space::state::PARTY_SIZE || self.planes.len()!=4*mt_engine::present::fb::PLANE {return Err("play: invalid space image size".into())}
        sp.s.image_mut().copy_from_slice(&self.ds);sp.s.party=self.party.clone();sp.s.world=self.world.clone();
        sp.clock=sp.input.play_clock(Box::new(mt_engine::sim::tick::Counter::at(self.ticks)));sp.clock.set(self.now);sp.last_ticks=self.last_ticks;
        sp.dead=self.dead;sp.scenario_entry=self.scenario_entry.clone();sp.mouse=self.mouse;sp.r.fb.planes=self.planes.clone();sp.r.fb.regs=self.regs;sp.dac=self.dac;sp.r.pen=self.pen;
        let frame=sp.input.play_checkpoint().map(|p|p.frame).unwrap_or_default();
        mt_engine::present::textlog::restore(&sp.r.fb,&self.text,&frame)?;
        sp.play_flight=Some(self.flight.clone());sp.product=true;sp.sounds.install(&self.sounds)?;Ok(())
    }
}

pub fn offer(sp:&mut Space,flight:space::flight::Flight) {
    sp.play_flight=Some(flight.clone());
    if sp.observer.is_some() || !sp.input.data_mode() || !sp.input.wants_decision() {return}
    let Some(input)=sp.input.play_checkpoint() else {return};
    let Some(parent)=&sp.play_ground else {return};
    let Ok(mut saved)=crate::save::Saved::decode(parent) else {return};
    let image=Image::capture(sp,flight);
    let Some(meta)=saved.play.as_mut() else {return};
    meta.input=input;meta.space=Some(image);
    sp.input.anchor("s.4FCB",&saved.encode());
}

impl Value for Image {
    fn put(&self,out:&mut Vec<u8>) {
        self.ds.put(out);self.party.put(out);self.world.put(out);self.ticks.put(out);self.now.put(out);self.last_ticks.put(out);
        self.dead.put(out);self.scenario_entry.put(out);self.mouse.put(out);self.planes.put(out);self.regs.put(out);self.dac.put(out);self.pen.put(out);self.flight.put(out);self.sounds.put(out);self.text.put(out);
    }
    fn get(input:&mut &[u8])->Result<Self,String> {
        Ok(Self {ds:Value::get(input)?,party:Value::get(input)?,world:Value::get(input)?,ticks:Value::get(input)?,now:Value::get(input)?,last_ticks:Value::get(input)?,
            dead:Value::get(input)?,scenario_entry:Value::get(input)?,mouse:Value::get(input)?,planes:Value::get(input)?,regs:Value::get(input)?,dac:Value::get(input)?,pen:Value::get(input)?,flight:Value::get(input)?,sounds:Value::get(input)?,text:Value::get(input)?})
    }
}
