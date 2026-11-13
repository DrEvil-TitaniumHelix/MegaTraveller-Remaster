use super::*;
use mt1::ui::Unit;

pub(super) fn story(d: &Decision, observed: &Observed) -> Value {
    let entries: Vec<_> = crate::plus::journal::entries().iter().map(|e| {
        if observed.story.unlocked.contains(e.id) {
            json!({"id":e.id,"unlocked":true,"text":e.text,"source":e.source,
                "candidate":e.rule.to_ascii_lowercase().contains("candidate")})
        } else { json!({"id":e.id,"unlocked":false}) }
    }).collect();
    json!({"ref":d.state,"entries":entries})
}
pub(super) fn help(d: &Decision, observed: &Observed) -> Result<Value, String> {
    let s = observed.state.as_ref().ok_or("no observed screen")?;
    Ok(json!({"ref":d.state,"text":crate::plus::help::text_with_decision(&s.scene,Some(d))}))
}

pub(super) fn value(d: &Decision, observed: &Observed) -> Value {
    let Some(o) = &observed.state else { return json!({"ref":d.state,"program":d.context.program}); };
    let s = &o.scene; let bytes = &s.party.0;
    let b = |at: usize| bytes.get(at).copied().unwrap_or(0);
    let word = |at: usize| u16::from_le_bytes([b(at), b(at+1)]);
    let money = |at: usize| u32::from_le_bytes([b(at), b(at+1), b(at+2), b(at+3)]);
    let string = |at: usize, max: usize| (at..at+max).map(b).take_while(|&v| v!=0).map(char::from).collect::<String>();
    let slice = |at: usize, len: usize| (at..at+len).map(b).collect::<Vec<_>>();
    let party: Vec<_> = (0..5u8).filter(|&i| s.party.mask() & (1 << i) != 0).map(|i| {
        let at = 0x102 + i as usize * 256;
        let gear: Vec<_> = (0..23).filter_map(|slot| {
            let n = word(at+0xCC+slot*2) as usize; if n == 0 || n > 256 { return None }
            let at = 0xA82+5*(n-1);
            Some(json!({"slot":slot,"item":b(at),"qty":b(at+1),"charge":b(at+2)}))
        }).collect();
        json!({"slot":i,"name":s.party.name(i),"hits":s.party.hits(i),"max_hits":s.party.max_hits(i),"alive":s.party.alive(i),
            "oxygen":s.party.oxygen(i),"attributes":slice(at+0x14,6),"skills":slice(at+0x29,80),"credits":money(at+0xFC),"gear":gear,
            "cell":o.travellers.iter().find(|(slot,_)|*slot==i).map(|(_,cell)|cell)})
    }).collect();
    let flight = o.flight.as_ref().map(|f| json!({"position":f.position,"velocity":f.velocity,"heading":f.heading,
        "thrust":f.thrust,"station":f.station,"encounter":f.encounter}));
    let position = if bytes.is_empty() { Value::Null } else {
        json!({"system":b(6),"world":if o.flight.is_none(){b(6)}else{b(7)},"area":if o.flight.is_none(){Some(b(7))}else{None},"level":if o.flight.is_none(){Some(s.level)}else{None},
            "cell":if o.flight.is_none(){Some((s.pos.0>>1,s.pos.1>>1))}else{None}})
    };
    let combat = if s.squad.encounter || o.encounter.is_some() {
        let npcs: Vec<_> = s.npcs.iter().enumerate().filter(|(_,n)| n.present() && n.level() as i8==s.level).map(|(i,n)|
            json!({"record":i+1,"cell":[n.x()>>1,n.y()>>1],"hits":o.npc_hits.get(&(i as u8+1)).copied().unwrap_or(Some(n.0[10] as i32)),"attitude":n.attitude()&3,"weapon":n.armed(),"ammo":n.0[30]})).collect();
        Some(json!({"id":o.encounter,"round":s.squad.round,"to_act":s.squad.to_act,"selected":s.squad.selected,
            "queue":s.squad.queue.iter().map(|u|match u {Unit::Traveller(i)=>format!("traveller/{i}"),Unit::Npc(i)=>format!("npc/{i}")}).collect::<Vec<_>>(),
            "pips":s.squad.pips,"movement":s.squad.mp,"tactics":s.squad.tactics,"npcs":npcs}))
    } else { None };
    json!({"ref":d.state,"program":d.context.program,"screen":format!("{:?}",s.screen).to_ascii_lowercase(),
        "party":party,"position":position,"time":{"tick":o.tick,"date":if bytes.is_empty(){None}else{Some(word(0xF84))}},
        "credits":if bytes.is_empty(){None}else{Some(money(0x88))},
        "ship":if bytes.is_empty(){Value::Null}else{json!({"name":string(0x8C,20),"fuel":b(0xFC),"unrefined_fuel":b(0xFD),
            "hull_damage":b(0xC6),"damage":slice(0xC2,4),"model":b(0xC0),"flags":b(0xC1),"cargo":slice(0xCA,20),
            "programs":slice(0xAC,20),"stations":slice(0xA0,12),"flight":flight})},
        "combat":combat,"messages":s.squad.log,"draft":o.draft.as_ref().map(|r|json!({"name":r.iter().take(20).take_while(|&&b|b!=0).map(|&b|char::from(b)).collect::<String>(),"record":r}))})
}
pub(super) fn events(before: &Value, after: &Value) -> Vec<Value> {
    let mut events = vec![];
    for (field, kind) in [("program","program"),("position","moved"),("credits","money"),("time","time")] {
        if before[field] != after[field] { events.push(json!({"type":kind,"before":before[field],"after":after[field]})); }
    }
    if let (Some(old),Some(new))=(before["party"].as_array(),after["party"].as_array()) {
        for traveller in new {
            if let Some(previous)=old.iter().find(|p|p["slot"]==traveller["slot"]) {
                if previous["credits"]!=traveller["credits"] {
                    events.push(json!({"type":"money","slot":traveller["slot"],"before":previous["credits"],"after":traveller["credits"]}));
                }
            }
        }
    }
    if fighting(after) && !fighting(before) { events.push(json!({"type":"encounter_start"})); }
    if !fighting(after) && fighting(before) { events.push(json!({"type":"encounter_end"})); }
    if before["messages"] != after["messages"] { events.push(json!({"type":"messages","text":after["messages"]})); }
    events
}
pub(super) fn fighting(s: &Value) -> bool {
    !s["combat"].is_null() || s.pointer("/ship/flight/encounter").and_then(Value::as_i64).is_some_and(|v| v >= 0)
}
pub(super) fn area(s: &Value) -> Value { json!([s["program"],s["position"]["system"],s["position"]["area"],s["position"]["level"]]) }
