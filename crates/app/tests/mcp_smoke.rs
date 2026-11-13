#[path = "../../mt1/tests/oracle_env/mod.rs"]
mod oracle_env;
use serde_json::{json,Value};
use std::{io::{BufRead,BufReader,Write},path::{Path,PathBuf},process::{Child,ChildStdin,Command,Stdio},sync::mpsc,time::Duration};

struct Client { child:Child, input:Option<ChildStdin>, output:mpsc::Receiver<String>, seq:u64, dir:PathBuf }
impl Client {
    fn new(gamefiles:Option<&Path>)->Self {
        let id=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir=Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/m8pmcp-tests/{}-{id}",std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut command=Command::new(env!("CARGO_BIN_EXE_megatraveller"));
        command.arg("--mcp").arg("--gamefiles").arg(gamefiles.unwrap_or(Path::new("missing-gamefiles"))).current_dir(&dir)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(std::fs::File::create(dir.join("stderr.log")).unwrap());
        let mut child=command.spawn().unwrap();let input=child.stdin.take();let stdout=child.stdout.take().unwrap();let (tx,output)=mpsc::channel();
        std::thread::spawn(move||{for line in BufReader::new(stdout).lines(){if tx.send(line.unwrap()).is_err(){break}}});
        Self{child,input,output,seq:0,dir}
    }
    fn send(&mut self,v:Value) {writeln!(self.input.as_mut().unwrap(),"{v}").unwrap();self.input.as_mut().unwrap().flush().unwrap()}
    fn read(&self)->Value {
        let line=self.output.recv_timeout(Duration::from_secs(90)).unwrap_or_else(|e|panic!("MCP response: {e}; stderr {}",self.dir.join("stderr.log").display()));
        serde_json::from_str(&line).unwrap_or_else(|e|panic!("non-JSON stdout {line:?}: {e}"))
    }
    fn rpc(&mut self,method:&str,params:Value)->Value {
        self.seq+=1;self.send(json!({"jsonrpc":"2.0","id":self.seq,"method":method,"params":params}));
        let v=self.read();assert_eq!(v["id"],self.seq,"duplicate/out-of-order response: {v}");assert_eq!(v["jsonrpc"],"2.0");v
    }
    fn init(&mut self)->Value {
        let r=self.rpc("initialize",json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"mcp-test","version":"1"}}));
        assert!(r.get("error").is_none(),"{r}");self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));r["result"].clone()
    }
    fn raw_tool(&mut self,name:&str,args:Value)->Value {self.rpc("tools/call",json!({"name":name,"arguments":args}))}
    fn tool(&mut self,name:&str,args:Value)->Value {
        let r=self.raw_tool(name,args);assert!(r.get("error").is_none(),"{name}: {r}");assert_eq!(r["result"]["isError"],false,"{name}: {r}");
        serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    }
    fn choose(&mut self,d:&Value,pick:Value)->Value {self.tool("choose",json!({"decision":d["id"],"pick":pick}))}
    fn eof(&mut self) {
        self.input.take();
        for _ in 0..250 {if let Some(status)=self.child.try_wait().unwrap(){assert!(status.success(),"{status}");return}std::thread::sleep(Duration::from_millis(20))}
        panic!("MCP did not exit on EOF")
    }
}
impl Drop for Client {fn drop(&mut self){let _=self.child.kill();let _=self.child.wait();}}
fn files()->Option<PathBuf> {oracle_env::gamefiles()}
fn scenario(name:&str)->String {Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../scenarios/{name}.toml")).canonicalize().unwrap().to_string_lossy().into_owned()}

fn unchanged(c:&mut Client, decision:&Value, state:&Value, trace:&Value) {
    assert_eq!(c.tool("decision",json!({}))["decision"],*decision);
    assert_eq!(c.tool("state",json!({})),*state);
    assert_eq!(c.tool("trace",json!({})),*trace);
}

#[test]
fn snapshot_build_matches_non_gui_printed_build_id() {
    let output=Command::new(env!("CARGO_BIN_EXE_megatraveller")).arg("--build-id").output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let build=String::from_utf8(output.stdout).unwrap(); let build=build.trim();
    assert_eq!(build, env!("MT_BUILD_HASH"));
    let Some(dir)=files() else{return};let mut c=Client::new(Some(&dir));c.init();
    c.tool("game_new",json!({"start":scenario("efate_quiet")}));
    assert_eq!(c.tool("snapshot_save",json!({"name":"Build identity"}))["build"],build);
    c.eof();
}

#[test]
fn journal_read_exposes_unlocked_entries_without_mutating_game() {
    let Some(dir)=files() else{return};let mut c=Client::new(Some(&dir));c.init();
    let first=c.tool("game_new",json!({"start":scenario("efate_quiet")})); let trace=c.tool("trace",json!({}));
    let journal=c.tool("journal",json!({}));
    assert_eq!(journal["ref"],first["state"]["ref"]);
    let entries=journal["entries"].as_array().unwrap(); assert_eq!(entries.len(),14);
    for id in ["J1","J2"] {let e=entries.iter().find(|e|e["id"]==id).unwrap();assert_eq!(e["unlocked"],true);assert!(!e["text"].as_str().unwrap().is_empty());}
    for id in ["J6","J13","J14"] {let e=entries.iter().find(|e|e["id"]==id).unwrap();assert_eq!(e["unlocked"],false);assert!(e.get("text").is_none(),"locked spoilers: {e}");}
    unchanged(&mut c,&first["decision"],&first["state"],&trace);
    assert_eq!(c.tool("journal",json!({})),journal);
    c.eof();
}

#[test]
fn help_read_is_contextual_without_mutating_game() {
    let Some(dir)=files() else{return};let mut c=Client::new(Some(&dir));c.init();
    for start in [scenario("efate_quiet"),"chargen".into()] {
        let first=c.tool("game_new",json!({"start":start}));let trace=c.tool("trace",json!({}));
        let help=c.tool("help",json!({}));assert_eq!(help["ref"],first["state"]["ref"]);
        let text=help["text"].as_str().unwrap();
        if first["decision"]["kind"]=="ground" { assert!(text.contains("E: End turn")&&text.contains("Guides"),"{text}"); }
        else { assert!(text.contains("Current screen:")&&text.contains("Available now:")&&text.contains("Menus / lists:"),"{text}"); }
        unchanged(&mut c,&first["decision"],&first["state"],&trace);
    }
    c.eof();
}

#[test]
fn snapshot_json_paths_cannot_escape_the_snapshot_directory() {
    let Some(dir)=files() else{return};let mut c=Client::new(Some(&dir));c.init();
    let first=c.tool("game_new",json!({"start":scenario("efate_quiet")}));
    let saved=c.tool("snapshot_save",json!({"name":"Confined"}));
    let root=c.dir.join("acceptance/local/play/snapshots");let filename=format!("{}.json",saved["id"].as_str().unwrap());
    let outside=c.dir.join(&filename);std::fs::copy(root.join(&filename),&outside).unwrap();
    let moved=c.choose(&first["decision"],json!("wait:1"));let trace=c.tool("trace",json!({}));
    for path in [outside,root.join("../../../..").join(&filename)] {
        let r=c.raw_tool("game_load",json!({"path":path}));assert_eq!(r["result"]["isError"],true,"outside snapshot loaded: {r}");
        unchanged(&mut c,&moved["decision"],&moved["state"],&trace);
    }
    for args in [json!({"snapshot":saved["id"]}),json!({"path":root.join(&filename)})] {
        let restored=c.tool("game_load",args);assert_eq!(restored["decision"],first["decision"]);assert_eq!(restored["state"],first["state"]);
    }
    c.eof();
}

#[test]
fn batch_requires_current_starting_decision_even_for_zero_picks_or_stops() {
    let Some(dir)=files() else{return};let mut c=Client::new(Some(&dir));c.init();
    let first=c.tool("game_new",json!({"start":scenario("efate_quiet")}));
    let moved=c.choose(&first["decision"],json!("wait:1"));let trace=c.tool("trace",json!({}));
    for (picks,stop) in [(json!(["wait:1"]),json!({})),(json!([]),json!({})),(json!(["wait:1"]),json!({"count":0})),(json!(["wait:1"]),json!({"kind":"ground"}))] {
        let r=c.raw_tool("choose_until",json!({"decision":first["decision"]["id"],"picks":picks,"stop":stop}));
        assert_eq!(r["result"]["isError"],true,"stale batch must be a domain refusal: {r}");assert!(r.to_string().contains("stale"),"{r}");
        unchanged(&mut c,&moved["decision"],&moved["state"],&trace);
    }
    assert_eq!(c.raw_tool("choose_until",json!({"picks":[]}))["error"]["code"],-32602);
    let accepted=c.tool("choose_until",json!({"decision":moved["decision"]["id"],"picks":["wait:1"]}));assert_eq!(accepted["applied"],1);
    c.eof();
}

#[test]
fn wounded_squad_health_is_observed_and_restored() {
    let Some(dir)=files() else{return};let mut c=Client::new(Some(&dir));c.init();
    let first=c.tool("game_new",json!({"start":scenario("m8c_save_encounter")}));
    let npcs=first["state"]["combat"]["npcs"].as_array().unwrap();
    assert_eq!(npcs.iter().find(|n|n["record"]==14).unwrap()["hits"],2);
    assert_eq!(first["state"]["party"][0]["hits"],12);
    let trace=c.tool("trace",json!({}));let snap=c.tool("snapshot_save",json!({"name":"Wounded squad"}));
    unchanged(&mut c,&first["decision"],&first["state"],&trace);
    c.tool("game_new",json!({"start":scenario("efate_quiet")}));
    let restored=c.tool("snapshot_restore",json!({"id":snap["id"]}));assert_eq!(restored["decision"],first["decision"]);assert_eq!(restored["state"],first["state"]);
    unchanged(&mut c,&first["decision"],&first["state"],&trace);c.eof();
}

#[test]
fn oversized_request_has_json_error_and_controlled_exit() {
    let mut c=Client::new(None);c.init();
    let request=json!({"jsonrpc":"2.0","id":99,"method":"ping"}).to_string();
    let mut line=request.into_bytes();line.resize((1<<20)-1,b' ');line.push(b'\n');
    c.input.as_mut().unwrap().write_all(&line).unwrap();c.input.as_mut().unwrap().flush().unwrap();assert_eq!(c.read()["result"],json!({}));
    line.insert(line.len()-1,b' ');
    c.input.as_mut().unwrap().write_all(&line).unwrap();c.input.as_mut().unwrap().flush().unwrap();
    let error=c.read();assert_eq!(error["jsonrpc"],"2.0");assert_eq!(error["id"],Value::Null);assert_eq!(error["error"]["code"],-32600);
    c.eof();assert!(c.output.recv_timeout(Duration::from_secs(1)).is_err(),"unexpected stdout after terminal error");
}

#[test]
fn invalid_json_rpc_id_types_are_refused_and_server_remains_responsive() {
    let mut c=Client::new(None);c.init();
    for id in [Value::Null,json!(true),json!(false),json!({"id":1}),json!([]),json!(1.5)] {
        c.send(json!({"jsonrpc":"2.0","id":id,"method":"ping"}));let error=c.read();
        assert_eq!(error["jsonrpc"],"2.0");assert_eq!(error["id"],Value::Null);assert_eq!(error["error"]["code"],-32600,"invalid id {id}: {error}");
        assert_eq!(c.rpc("ping",json!({}))["result"],json!({}));
    }
    for id in [json!("request"),json!(-1),json!(u64::MAX)] {
        c.send(json!({"jsonrpc":"2.0","id":id,"method":"ping"}));let r=c.read();assert_eq!(r["id"],id);assert_eq!(r["result"],json!({}));
    }
    c.eof();
}

#[test]
fn batch_over_100_picks_is_refused_without_mutation() {
    let Some(dir)=files() else{return};let mut c=Client::new(Some(&dir));c.init();
    let first=c.tool("game_new",json!({"start":scenario("efate_quiet")}));let trace=c.tool("trace",json!({}));
    let args=|count|json!({"decision":first["decision"]["id"],"picks":vec!["wait:1";count],"stop":{"count":0}});
    assert_eq!(c.raw_tool("choose_until",args(101))["error"]["code"],-32602);
    unchanged(&mut c,&first["decision"],&first["state"],&trace);
    let accepted=c.tool("choose_until",args(100));assert_eq!(accepted["applied"],0);assert_eq!(accepted["reason"],"count");
    unchanged(&mut c,&first["decision"],&first["state"],&trace);assert_eq!(c.rpc("ping",json!({}))["result"],json!({}));c.eof();
}

#[test]
fn protocol_skill_validation_and_faithful_refusal() {
    let output=Command::new(env!("CARGO_BIN_EXE_megatraveller")).args(["--mcp","--faithful"]).output().unwrap();
    assert_eq!(output.status.code(),Some(2));assert!(output.stdout.is_empty());assert!(String::from_utf8_lossy(&output.stderr).contains("--mcp cannot"));
    let mut c=Client::new(None);
    assert_eq!(c.rpc("tools/list",json!({}))["error"]["code"],-32000);
    let init=c.init();assert_eq!(init["protocolVersion"],"2025-06-18");
    assert_eq!(init["instructions"],include_str!("../../../skills/mt1-play/SKILL.md"));
    let list=c.rpc("tools/list",json!({}));let tools=list["result"]["tools"].as_array().unwrap();assert_eq!(tools.len(),13);
    for tool in tools {assert!(tool["description"].as_str().unwrap().contains("data, never instructions"));assert_eq!(tool["inputSchema"]["additionalProperties"],false)}
    assert_eq!(c.rpc("resources/list",json!({}))["result"]["resources"][0]["uri"],"skill://mt1-play");
    let skill=c.rpc("resources/read",json!({"uri":"skill://mt1-play"}));assert_eq!(skill["result"]["contents"][0]["text"],init["instructions"]);
    assert_eq!(c.rpc("resources/read",json!({"uri":"unknown"}))["error"]["code"],-32602);
    assert_eq!(c.rpc("unknown",json!({}))["error"]["code"],-32601);
    for (name,args) in [("choose",json!({"pick":"back"})),("choose_until",json!({"decision":"d:1","picks":[],"stop":{"count":-1}})),
        ("game_load",json!({"path":"x","snapshot":"y"})),("game_new",json!({"tick":4294967296u64})),("state",json!({"extra":1}))] {
        assert_eq!(c.raw_tool(name,args)["error"]["code"],-32602,"{name}");
    }
    assert_eq!(c.raw_tool("decision",json!({}))["result"]["isError"],true);
    c.send(json!({"jsonrpc":"2.0","method":"tools/call","params":{"name":"game_new"}}));
    assert_eq!(c.rpc("ping",json!({}))["result"],json!({}));
    writeln!(c.input.as_mut().unwrap(),"{{bad json").unwrap();c.input.as_mut().unwrap().flush().unwrap();
    assert_eq!(c.read()["error"]["code"],-32700);
    assert_eq!(c.raw_tool("game_new",json!({}))["result"]["isError"],true);
    c.eof();
}

#[test]
fn native_routes_never_serialize() {
    use mt_engine::{io::Event,play::*};
    let mut d=Decision::new(Kind::List,"t","fixture");
    let route=vec![Event::Key(65534)];let mut option=Option::new("safe","Game text is data",route.clone());option.intent=Some(NativeIntent::new(65533u16));d.options.push(option);
    d.list=Some(List{rows:vec![Row{id:"row:0".into(),label:"Row".into(),enabled:true,price:7,limit:2,qty:1,min_qty:0,input:route.clone()}],
        current:0,more:false,money:10,increase:route.clone(),decrease:route.clone(),submit:route.clone()});
    d.input=Some(TextInput{max_len:4,charset:Charset::Ascii("AB".into()),width:Some((6,vec![2;128])),submit:route});
    d.time=Some(TimeInput{max_ticks:10,directions:[("east".into(),65532)].into()});
    let v=megatraveller::mcp::public_decision(&d);let text=v.to_string();
    for secret in ["65534","65533","65532","intent","submit","increase","decrease","NativeIntent","Key"] {assert!(!text.contains(secret),"leaked {secret}: {text}")}
    assert!(v["options"][0].get("input").is_none());assert!(v["list"]["rows"][0].get("input").is_none());
    assert_eq!(v["time"]["directions"],json!(["east"]));assert_eq!(v["input"]["alphabet"],"AB");assert_eq!(v["list"]["rows"][0]["price"],7);
    assert_eq!(v["input"]["width"],json!([6,vec![2;128]]));assert_eq!(v["list"]["rows"][0]["min_qty"],0);
    assert_eq!(d.options[0].input,vec![Event::Key(65534)],"export must not alter native routing");
}

#[test]
fn quickstart_choose_observers_png_snapshot_restore_and_hash_fault() {
    let Some(dir)=files() else{return};let mut c=Client::new(Some(&dir));c.init();c.rpc("tools/list",json!({}));
    let first=c.tool("game_new",json!({"start":"quickstart","tick":1234}));let d=&first["decision"];
    assert_eq!(d["kind"],"ack"); assert_eq!(first["state"]["screen"],"intro","quickstart enters the intro directly");
    assert_eq!(c.tool("decision",json!({}))["decision"],*d);
    let pick=d["options"].as_array().unwrap().iter().find(|o|o["enabled"]==true&&o["id"]=="continue").unwrap()["id"].clone();
    let next=c.choose(d,pick);let expected=next["decision"].clone();let expected_state=c.tool("state",json!({}));
    let before=c.tool("trace",json!({}));
    let image=c.raw_tool("view",json!({}));assert_eq!(image["result"]["content"][0]["mimeType"],"image/png");
    let data=image["result"]["content"][0]["data"].as_str().unwrap();
    let mut python=Command::new("python").args(["-B","-c",r#"import sys,base64,struct,zlib
b=base64.b64decode(sys.stdin.read(),validate=True); assert b[:8]==b'\x89PNG\r\n\x1a\n'
p=8; compressed=b''
while p<len(b):
 n=struct.unpack('>I',b[p:p+4])[0]; tag=b[p+4:p+8]; data=b[p+8:p+8+n]; crc=struct.unpack('>I',b[p+8+n:p+12+n])[0]
 assert zlib.crc32(tag+data)==crc
 if tag==b'IHDR': assert struct.unpack('>IIBBBBB',data)==(320,200,8,2,0,0,0)
 if tag==b'IDAT': compressed+=data
 p+=n+12
raw=zlib.decompress(compressed); assert len(raw)==200*961; assert all(raw[i*961]==0 for i in range(200))
"#]).stdin(Stdio::piped()).spawn().unwrap();python.stdin.take().unwrap().write_all(data.as_bytes()).unwrap();assert!(python.wait().unwrap().success());
    let saved=c.tool("snapshot_save",json!({"name":"Smoke snapshot"}));let snapshots=c.tool("snapshot_list",json!({}));
    assert!(snapshots["snapshots"].as_array().unwrap().iter().any(|s|s["id"]==saved["id"]));
    assert_eq!(c.tool("state",json!({})),expected_state);assert_eq!(c.tool("trace",json!({})),before);
    assert_eq!(c.raw_tool("choose",json!({"decision":"d:0","pick":"continue"}))["result"]["isError"],true);
    assert_eq!(c.tool("decision",json!({}))["decision"],expected);
    let restored=c.tool("snapshot_restore",json!({"id":saved["id"]}));assert_eq!(restored["decision"],expected);assert_eq!(restored["state"],expected_state);
    let loaded=c.tool("game_load",json!({"snapshot":saved["id"]}));assert_eq!(loaded["decision"],expected);
    let path=c.dir.join("acceptance/local/play/snapshots").join(format!("{}.json",saved["id"].as_str().unwrap()));
    let loaded=c.tool("game_load",json!({"path":path}));assert_eq!(loaded["decision"],expected);
    assert_eq!(c.raw_tool("snapshot_restore",json!({"id":"../escape"}))["result"]["isError"],true);
    assert_eq!(c.tool("decision",json!({}))["decision"],expected);
    let mut document:Value=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    document["hash"]=json!("s:0000000000000000");
    if let Some(last)=document["journal"]["entries"].as_array_mut().unwrap().last_mut(){last["after"]=json!("s:0000000000000000")}
    std::fs::write(&path,serde_json::to_vec(&document).unwrap()).unwrap();
    let error=c.raw_tool("snapshot_restore",json!({"id":saved["id"]}));assert_eq!(error["result"]["isError"],true);assert!(error.to_string().contains("ReplayMismatch"));
    c.eof();
}

#[test]
fn batch_count_kind_refusal_and_combat_stops() {
    let Some(dir)=files() else{return};let mut c=Client::new(Some(&dir));c.init();
    let first=c.tool("game_new",json!({"start":scenario("efate_quiet")}));assert_eq!(first["decision"]["kind"],"ground");
    assert_eq!(first["state"]["position"]["cell"],json!([32,9]));assert_eq!(first["state"]["party"].as_array().unwrap().len(),5);
    let stopped=c.tool("choose_until",json!({"decision":first["decision"]["id"],"picks":["wait:1"],"stop":{"kind":"ground"}}));assert_eq!(stopped["reason"],"kind");assert_eq!(stopped["applied"],0);assert_eq!(stopped["decision"],first["decision"]);
    let moved=c.tool("choose_until",json!({"decision":first["decision"]["id"],"picks":["wait:1","wait:1","wait:1"],"stop":{"count":2}}));
    assert_eq!(moved["applied"],2);assert_eq!(moved["reason"],"count");assert_eq!(moved["grants"].as_array().unwrap().len(),2);
    for grant in moved["grants"].as_array().unwrap(){assert_eq!(grant["requested"],1);assert_eq!(grant["used"],1);assert_eq!(grant["remaining"],0)}
    let menu=c.tool("choose_until",json!({"decision":moved["decision"]["id"],"picks":["cmd:game"],"stop":{"kind":"menu"}}));assert_eq!(menu["applied"],1);assert_eq!(menu["reason"],"kind","stop checked after last pick");
    let options=menu["decision"]["options"].as_array().unwrap();assert!(options.iter().all(|o|o["id"]!="back"),"{options:?}");
    let row=options.iter().find(|o|o["label"]=="Back").unwrap_or_else(||panic!("no Back row: {options:?}"))["id"].clone();
    let back=c.choose(&menu["decision"],row);assert_eq!(back["decision"]["kind"],"ground");
    let error=c.raw_tool("choose_until",json!({"decision":back["decision"]["id"],"picks":["wait:1","not-an-option","wait:1"]}));assert_eq!(error["result"]["isError"],true);
    let body:Value=serde_json::from_str(error["result"]["content"][0]["text"].as_str().unwrap()).unwrap();assert_eq!(body["applied"],1);assert_eq!(body["reason"],"refused");
    let combat=c.tool("game_new",json!({"start":scenario("efate_broken")}));assert_eq!(combat["decision"]["kind"],"combat.turn");
    let stopped=c.tool("choose_until",json!({"decision":combat["decision"]["id"],"picks":["end_turn"]}));assert_eq!(stopped["reason"],"combat");assert_eq!(stopped["applied"],0);assert_eq!(stopped["decision"],combat["decision"]);
    c.eof();
}

#[test]
fn chargen_title_and_save_roots_replace_sessions() {
    let Some(dir)=files() else{return};let mut c=Client::new(Some(&dir));c.init();
    let cg=c.tool("game_new",json!({"start":"chargen","tick":77}));assert_eq!(cg["decision"]["context"]["program"],"cg");
    let snap=c.tool("snapshot_save",json!({"name":"CG"}));
    let boot=c.tool("game_new",json!({"start":"boot","tick":77}));assert_eq!(boot["decision"]["context"]["program"],"title");
    let cg2=c.tool("snapshot_restore",json!({"id":snap["id"]}));assert_eq!(cg2["decision"],cg["decision"]);
    assert_eq!(c.raw_tool("game_new",json!({"start":"missing-scenario"}))["result"]["isError"],true);
    assert_eq!(c.tool("decision",json!({}))["decision"],cg["decision"]);
    let save=c.dir.join("test.mtsav");
    let result=Command::new(env!("CARGO_BIN_EXE_megatraveller")).arg("--gamefiles").arg(&dir).arg("--scenario").arg(scenario("efate_quiet")).arg("--scenario-check").arg("--save").arg(&save).output().unwrap();assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
    let loaded=c.tool("game_load",json!({"path":save}));assert_eq!(loaded["decision"]["context"]["program"],"t");assert_eq!(loaded["state"]["position"]["cell"],json!([32,9]));
    let snapshot=c.tool("snapshot_save",json!({"name":"Ground anchor"}));
    let restored=c.tool("snapshot_restore",json!({"id":snapshot["id"]}));assert_eq!(restored["decision"],loaded["decision"]);assert_eq!(restored["state"],loaded["state"]);
    let moved=c.choose(&restored["decision"],json!("move:east:1"));let next=c.choose(&moved["decision"],json!("wait:1"));
    let snapshot=c.tool("snapshot_save",json!({"name":"Initialized ground anchor"}));
    let path=c.dir.join("acceptance/local/play/snapshots").join(format!("{}.json",snapshot["id"].as_str().unwrap()));
    let document:Value=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();assert_eq!(document["root"]["kind"],"anchor");
    let restored=c.tool("snapshot_restore",json!({"id":snapshot["id"]}));assert_eq!(restored["decision"],next["decision"]);assert_eq!(restored["state"],next["state"]);
    let image=c.raw_tool("view",json!({}));assert_eq!(image["result"]["content"][0]["mimeType"],"image/png");
    assert_eq!(c.tool("decision",json!({}))["decision"],next["decision"]);
    c.eof();
}

#[test]
fn launch_observation_and_space_snapshot_replay_pending_scenario_fields() {
    let Some(dir)=files() else{return};let mut c=Client::new(Some(&dir));c.init();
    let fixture=c.dir.join("jump.toml");
    let text=std::fs::read_to_string(scenario("m6_jumpscreen_louzy")).unwrap().replace("cell = [33, 4]","cell = [32, 9]");
    std::fs::write(&fixture,text).unwrap();
    let ground=c.tool("game_new",json!({"start":fixture}));assert_eq!(ground["state"]["program"],"t");
    let space=c.tool("choose_until",json!({"decision":ground["decision"]["id"],"picks":["move:north:100"],"stop":{"program":true,"combat":false}}));
    assert_eq!(space["reason"],"program");assert_eq!(space["applied"],1);assert_eq!(space["state"]["program"],"s");
    assert_eq!(space["state"]["position"]["system"],2);assert!(!space["state"]["ship"]["flight"].is_null());
    let snapshot=c.tool("snapshot_save",json!({"name":"In space"}));
    let restored=c.tool("snapshot_restore",json!({"id":snapshot["id"]}));assert_eq!(restored["decision"],space["decision"]);assert_eq!(restored["state"],space["state"]);
    c.eof();
}

#[test]
fn zz_skip_report() { oracle_env::report() }
