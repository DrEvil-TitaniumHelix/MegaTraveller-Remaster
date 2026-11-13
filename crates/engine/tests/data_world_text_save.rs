mod common;
use common::{files_with_suffix, gamefiles, oracle_json, J};
use mt_engine::data::world::{Area, AreaHdr, Chunk, Map, Spawn, WObj};
use mt_engine::data::chars::Char;
use mt_engine::data::sav::{Sav, SavHdr, SavObj};
use mt_engine::data::{chars, dict, itm, msg, sav, world, Variant};
use std::path::PathBuf;

const V: Variant = Variant::Mt1;
fn ints(b: &[u8]) -> J { J::arr(b.iter().map(|&x| x as i64)) }
fn u16s(b: &[u16]) -> J { J::arr(b.iter().map(|&x| x as i64)) }
fn hexes<'a>(it: impl IntoIterator<Item = &'a [u8]>) -> J { J::arr(it.into_iter().map(J::from)) }

fn j_map(m: &Map) -> J {
    J::obj(vec![("hdr", J::obj(vec![("rows", m.hdr.rows.into()), ("cols", m.hdr.cols.into()), ("name", m.hdr.name.into()), ("hdr_rest", m.hdr.hdr_rest.into())])),
        ("cells", J::arr(m.cells.iter().map(|c| ints(c))))])
}
fn j_area_hdr(h: &AreaHdr) -> J {
    J::obj(vec![("f00", h.f00.into()), ("scene_ids", ints(&h.scene_ids)), ("uwp", ints(&h.uwp)), ("f15", h.f15.into()), ("f16", h.f16.into()),
        ("level_mask", h.level_mask.into()), ("f18", h.f18.into()), ("ly_a", h.ly_a.into()), ("ly_b", h.ly_b.into()), ("spawn_base", h.spawn_base.into()),
        ("pic_ids", ints(&h.pic_ids)), ("start_x", h.start_x.into()), ("start_y", h.start_y.into()), ("description", h.description.into()),
        ("slogan", h.slogan.into()), ("text3", h.text3.into()), ("place_names", hexes(h.place_names.iter().map(|p| &p[..])))])
}
fn j_wobj(o: &WObj) -> J {
    J::obj(vec![("id", o.id.into()), ("state", o.state.into()), ("x", o.x.into()), ("y", o.y.into()), ("type", o.r#type.into()), ("f8", o.f8.into()),
        ("f9", o.f9.into()), ("fa", o.fa.into()), ("fb", o.fb.into()), ("fc", o.fc.into()), ("f1a", o.f1a.into()), ("f1c", o.f1c.into())])
}
fn j_spawn(s: &Spawn) -> J { J::obj(vec![("level", s.level.into()), ("item", s.item.into()), ("x", s.x.into()), ("y", s.y.into())]) }
fn j_area(a: &Area) -> J {
    J::obj(vec![("hdr", j_area_hdr(&a.hdr)), ("level_offs", u16s(&a.level_offs)), ("map_offs", J::arr(a.map_offs.iter().map(|&o| o as i64))),
        ("maps", J::arr(a.maps.iter().map(j_map))), ("objects", J::arr(a.objects.iter().map(j_wobj))), ("spawns", J::arr(a.spawns.iter().map(j_spawn)))])
}
fn j_world(w: &[Chunk]) -> J { J::arr(w.iter().map(|c| match c { Chunk::Inside(m) => J::obj(vec![("inside", j_map(m))]), Chunk::Area(a) => j_area(a) })) }
fn j_dict(d: &dict::Dict) -> J { J::obj(vec![("words", hexes(d.words.iter().map(|w| &w[..]))), ("tail", J::from(&d.tail[..]))]) }
fn j_msg(m: &msg::Msgs) -> J { J::obj(vec![("msgs", J::arr(m.msgs.iter().map(|t| J::arr(t.iter().map(|&x| x as i64))))), ("tail", J::from(&m.tail[..]))]) }
fn j_item(i: &itm::Item) -> J {
    J::obj(vec![("name", i.name.into()), ("type", i.r#type.into()), ("weight", i.weight.into()), ("price", i.price.into()), ("f25", i.f25.into()),
        ("icon", i.icon.into()), ("ammo_flags", i.ammo_flags.into()), ("charges", i.charges.into()), ("f29", i.f29.into()), ("f2a", i.f2a.into()),
        ("tech_level", i.tech_level.into()), ("law_level", i.law_level.into()), ("f2d", i.f2d.into())])
}
fn j_char(c: &Char) -> J {
    J::obj(vec![("name", c.name.into()), ("attr", ints(&c.attr)), ("attr_base", ints(&c.attr_base)), ("service", c.service.into()), ("rank", c.rank.into()),
        ("age", c.age.into()), ("hits_max", c.hits_max.into()), ("hits_split", c.hits_split.into()), ("hits", c.hits.into()), ("oxygen", c.oxygen.into()),
        ("pension", c.pension.into()), ("flags", c.flags.into()), ("skill0", c.skill0.into()), ("skills", ints(&c.skills)), ("skills_unmapped", ints(&c.skills_unmapped)),
        ("skill_progress", ints(&c.skill_progress)), ("state_c9", c.state_c9.into()), ("f_ca", c.f_ca.into()), ("f_cb", c.f_cb.into()),
        ("inventory", u16s(&c.inventory)), ("actor", c.actor.into()), ("armor", c.armor.into()), ("weapon", c.weapon.into()), ("f_f8", c.f_f8.into()),
        ("f_fa", c.f_fa.into()), ("credits", c.credits.into())])
}
fn j_sav_hdr(h: &SavHdr) -> J {
    J::obj(vec![("alive_mask", h.alive_mask.into()), ("order", ints(&h.order)), ("world", h.world.into()), ("area", h.area.into()),
        ("object_bits", ints(&h.object_bits)), ("npc_bits", ints(&h.npc_bits)), ("spawn_bits", ints(&h.spawn_bits)), ("flag_81", h.flag_81.into()),
        ("air_counter", h.air_counter.into()), ("atmosphere_timer", h.atmosphere_timer.into()), ("hw_config", ints(&h.hw_config)), ("money", h.money.into()),
        ("ship_name", h.ship_name.into()), ("stations", ints(&h.stations)), ("programs", ints(&h.programs)), ("ship_model", h.ship_model.into()),
        ("ship_flags", h.ship_flags.into()), ("ship_damage", ints(&h.ship_damage)), ("hull_damage", h.hull_damage.into()), ("f_c7", h.f_c7.into()),
        ("gun_bays", ints(&h.gun_bays)), ("cargo", ints(&h.cargo)), ("supplies", ints(&h.supplies)), ("f_f2", ints(&h.f_f2)), ("fuel", h.fuel.into()),
        ("fuel_refined", h.fuel_refined.into()), ("ship_stats", ints(&h.ship_stats))])
}
fn j_sav_obj(o: &SavObj) -> J { J::obj(vec![("item", o.item.into()), ("qty", o.qty.into()), ("charge", o.charge.into()), ("f3", o.f3.into()), ("f4", o.f4.into())]) }
fn j_sav(s: &Sav) -> J {
    J::obj(vec![("hdr", j_sav_hdr(&s.hdr)), ("chars", J::arr(s.chars.iter().map(j_char))), ("unreferenced_902", s.unreferenced_902.into()),
        ("objects", J::arr(s.objects.iter().map(j_sav_obj))), ("lights", s.lights.into()), ("clock", s.clock.into())])
}

fn named(names: &[&str]) -> Option<Vec<PathBuf>> {
    let g = gamefiles()?;
    let v: Vec<PathBuf> = names.iter().map(|n| g.join(n)).filter(|p| p.is_file()).collect();
    if v.is_empty() { println!("SKIP: none of {names:?} in {}", g.display()); }
    Some(v)
}
fn suffixed(suffix: &str) -> Option<Vec<PathBuf>> {
    let g = gamefiles()?; let v = files_with_suffix(&g, suffix);
    if v.is_empty() { println!("SKIP: no *{suffix} in {}", g.display()); }
    Some(v)
}
fn round_trip<T>(files: Option<Vec<PathBuf>>, dec: impl Fn(&[u8]) -> mt_engine::data::bin::Result<T>, enc: impl Fn(&T) -> Vec<u8>) {
    for f in files.unwrap_or_default() {
        let d = std::fs::read(&f).unwrap();
        let v = dec(&d).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let e = enc(&v);
        let first = d.iter().zip(&e).position(|(a, b)| a != b).unwrap_or(d.len().min(e.len()));
        assert!(e == d, "{}: round trip differs (orig {} B, enc {} B, first diff at {first})", f.display(), d.len(), e.len());
        println!("PASS {}", f.display());
    }
}
fn cross_check<T>(files: Option<Vec<PathBuf>>, fmt: &str, dec: impl Fn(&[u8]) -> mt_engine::data::bin::Result<T>, json: impl Fn(&T) -> J) {
    for f in files.unwrap_or_default() {
        let Some(want) = oracle_json("m1_dump_wts.py", fmt, &f) else { return };
        let got = json(&dec(&std::fs::read(&f).unwrap()).unwrap()).to_string();
        if got != want {
            let i = got.bytes().zip(want.bytes()).position(|(a, b)| a != b).unwrap_or(got.len().min(want.len()));
            let (a, b) = (i.saturating_sub(60), i + 60);
            panic!("{}: JSON differs at byte {i}\n rust:   {}\n oracle: {}", f.display(), &got[a..b.min(got.len())], &want[a..b.min(want.len())]);
        }
        println!("PASS {} (oracle cross-check {fmt})", f.display());
    }
}

#[test] fn world_round_trip() { round_trip(named(&["W.DAT"]), |d| world::decode(V, d), |v| world::encode(V, v).unwrap()) }
#[test] fn world_cross_check() { cross_check(named(&["W.DAT"]), "W.DAT", |d| world::decode(V, d), |v| j_world(v)) }
#[test] fn dict_round_trip() { round_trip(named(&["TRAV.DAT"]), dict::decode, dict::encode) }
#[test] fn dict_cross_check() { cross_check(named(&["TRAV.DAT"]), "TRAV.DAT", dict::decode, j_dict) }
#[test] fn msg_round_trip() { round_trip(named(&["TRAV.MSG"]), |d| msg::decode(V, d), |v| msg::encode(V, v).unwrap()) }
#[test] fn msg_cross_check() { cross_check(named(&["TRAV.MSG"]), "TRAV.MSG", |d| msg::decode(V, d), j_msg) }
#[test] fn itm_round_trip() { round_trip(named(&["TRAV.ITM"]), |d| itm::decode(V, d), |v| itm::encode(V, v).unwrap()) }
#[test] fn itm_cross_check() { cross_check(named(&["TRAV.ITM"]), "TRAV.ITM", |d| itm::decode(V, d), |v| J::arr(v.iter().map(j_item))) }
#[test] fn chars_round_trip() { round_trip(suffixed("CHARS.DAT"), |d| chars::decode(V, d), |v| chars::encode(V, v).unwrap()) }
#[test] fn chars_cross_check() { cross_check(suffixed("CHARS.DAT"), "CHARS", |d| chars::decode(V, d), |v| J::arr(v.iter().map(j_char))) }
#[test] fn sav_round_trip() { round_trip(suffixed(".SAV"), |d| sav::decode(V, d), |v| sav::encode(V, v).unwrap()) }
#[test] fn sav_cross_check() { cross_check(suffixed(".SAV"), "SAV", |d| sav::decode(V, d), j_sav) }
