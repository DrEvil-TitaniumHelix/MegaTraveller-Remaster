use std::{fs, path::Path};
use syn::{visit::{self, Visit}, *};

fn dev(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| a.path().is_ident("cfg") && a.parse_args::<Meta>().is_ok_and(dev_meta))
}
fn dev_meta(meta: Meta) -> bool {
    matches!(meta, Meta::NameValue(v) if v.path.is_ident("feature") && matches!(&v.value, Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) if s.value() == "dev"))
}
fn player(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| a.path().is_ident("cfg") && a.parse_args::<Meta>().is_ok_and(|meta| {
        matches!(meta, Meta::List(list) if list.path.is_ident("not") && list.parse_args::<Meta>().is_ok_and(dev_meta))
    }))
}
struct Host;
impl<'a> Visit<'a> for Host {
    fn visit_item_mod(&mut self, n: &'a ItemMod) {
        if dev(&n.attrs) { return; }
        if n.ident == "bridge" { assert!(n.content.is_some(), "bridge implementation module lost its dev gate"); }
        visit::visit_item_mod(self, n);
    }
    fn visit_impl_item_fn(&mut self, n: &'a ImplItemFn) { if !dev(&n.attrs) { visit::visit_impl_item_fn(self, n); } }
    fn visit_item_fn(&mut self, n: &'a ItemFn) { if !dev(&n.attrs) { visit::visit_item_fn(self, n); } }
    fn visit_local(&mut self, n: &'a Local) { if !dev(&n.attrs) { visit::visit_local(self, n); } }
    fn visit_field(&mut self, n: &'a Field) {
        if dev(&n.attrs) { return; }
        assert!(n.ident.as_ref().is_none_or(|n| n != "bridge"), "bridge state lost its dev gate"); visit::visit_field(self, n);
    }
    fn visit_expr_if(&mut self, n: &'a ExprIf) { if !dev(&n.attrs) { visit::visit_expr_if(self, n); } }
    fn visit_expr_block(&mut self, n: &'a ExprBlock) { if !dev(&n.attrs) { visit::visit_expr_block(self, n); } }
    fn visit_expr_method_call(&mut self, n: &'a ExprMethodCall) {
        if dev(&n.attrs) { return; }
        assert!(!n.method.to_string().starts_with("bridge_"), "bridge host call lost its dev gate"); visit::visit_expr_method_call(self, n);
    }
    fn visit_member(&mut self, n: &'a Member) { assert!(!matches!(n, Member::Named(i) if i == "bridge"), "bridge field access lost its dev gate"); }
    fn visit_path(&mut self, n: &'a syn::Path) { assert!(!n.segments.iter().any(|s| s.ident == "bridge"), "bridge reference lost its dev gate"); }
}
struct Marker(bool);
impl<'a> Visit<'a> for Marker {
    fn visit_expr_call(&mut self, n: &'a ExprCall) {
        if matches!(n.func.as_ref(), Expr::Path(p) if p.path.is_ident("dev_only")) { self.0 = true; }
        visit::visit_expr_call(self, n);
    }
}
struct Entries;
impl Entries {
    fn check(&self, name: &Ident, body: &Block) {
        let mut marker = Marker(false); marker.visit_block(body);
        assert!(marker.0, "bridge entry {name} lost its structural marker");
    }
}
impl<'a> Visit<'a> for Entries {
    fn visit_item_fn(&mut self, n: &'a ItemFn) {
        if !matches!(n.vis, Visibility::Inherited) || ["send", "status", "note", "accept", "discard", "read_json", "write_json"].contains(&n.sig.ident.to_string().as_str()) {
            self.check(&n.sig.ident, &n.block);
        }
        visit::visit_item_fn(self, n);
    }
    fn visit_impl_item_fn(&mut self, n: &'a ImplItemFn) {
        if !matches!(n.vis, Visibility::Inherited) || n.sig.ident == "run" { self.check(&n.sig.ident, &n.block); }
        visit::visit_impl_item_fn(self, n);
    }
}
#[test]
fn implementation_and_every_host_hook_require_dev() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for name in ["plus/mod.rs", "window.rs", "window/plus.rs", "panels/plus.rs"] {
        Host.visit_file(&syn::parse_file(&fs::read_to_string(src.join(name)).unwrap()).unwrap());
    }
    let implementation = fs::read_to_string(src.join("plus/bridge.rs")).unwrap();
    assert!(syn::parse_file(&implementation).unwrap().items.iter().any(|item| {
        matches!(item, Item::Macro(m) if m.mac.path.is_ident("compile_error") && player(&m.attrs))
    }), "independent implementation gate missing");
    for name in ["plus/bridge.rs", "plus/bridge/files.rs", "plus/bridge/process.rs"] {
        Entries.visit_file(&syn::parse_file(&fs::read_to_string(src.join(name)).unwrap()).unwrap());
    }
}
