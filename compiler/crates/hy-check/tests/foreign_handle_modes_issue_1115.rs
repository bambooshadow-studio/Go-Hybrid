//! Regression tests for #1115: `serde_json.new_array` / `push_string` handle
//! modes, curated in-place mutators, and unproven non-scalar atomics.
//!
//! Each case is an accepted-but-unsound program that the foreign descriptor
//! stubs used to describe too permissively.

use hy_check::{check_file, CheckResult, Verdict};
use hy_syntax::parse_file;

fn check(src: &str) -> CheckResult {
    let ast = parse_file("test.hy", src).expect("test snippet must parse");
    check_file(&ast)
}

fn codes(r: &CheckResult) -> Vec<String> {
    r.diagnostics.iter().map(|d| d.code.clone()).collect()
}

#[test]
fn new_array_result_is_an_own_handle_and_a2_conflict_is_rejected() {
    let r = check(
        r#"import "rust/serde_json"
fn main() {
    arr := serde_json.new_array()
    elem := arr.get("0")?
    arr.set("0", "x")
    fmt.println(elem.string("0")?)
}
"#,
    );
    assert_eq!(r.verdict, Verdict::Reject, "got {:?}", r.diagnostics);
    assert!(codes(&r).iter().any(|c| c == "HYB-E3101"));
}

#[test]
fn new_array_view_escaping_into_struct_field_is_e3103() {
    let r = check(
        r#"import "rust/serde_json"
struct holder {
    v serde_json.view
}
fn main() {
    arr := serde_json.new_array()
    elem := arr.get("0")?
    h := holder{v: elem}
    fmt.println(elem.string("name")?)
    _ = h
}
"#,
    );
    assert_eq!(r.verdict, Verdict::Reject, "got {:?}", r.diagnostics);
    assert!(codes(&r).iter().any(|c| c == "HYB-E3103"));
}

#[test]
fn push_string_method_and_package_forms_conflict_with_live_view() {
    let method_form = check(
        r#"import "rust/serde_json"
fn main() {
    doc := serde_json.parse(`{"items":["a"]}`)?
    v := doc.array("items")?
    doc.push_string("b")
    fmt.println(v.string("0")?)
}
"#,
    );
    assert_eq!(
        method_form.verdict,
        Verdict::Reject,
        "got {:?}",
        method_form.diagnostics
    );
    assert!(codes(&method_form).iter().any(|c| c == "HYB-E3101"));

    let package_form = check(
        r#"import "rust/serde_json"
fn main() {
    doc := serde_json.parse(`{"items":["a"]}`)?
    v := serde_json.get(doc, "items")?
    serde_json.push_string(doc, "c")
    fmt.println(v.string("0")?)
}
"#,
    );
    assert_eq!(
        package_form.verdict,
        Verdict::Reject,
        "got {:?}",
        package_form.diagnostics
    );
    assert!(codes(&package_form).iter().any(|c| c == "HYB-E3101"));
}

#[test]
fn curated_in_place_mutators_on_const_roots_are_e3006() {
    let r = check(
        r#"import "encoding/hex"
import "encoding/json"

struct cfg {
    port int
}

struct slot64 {
    v int64
}

const dst []byte = [0, 0, 0, 0]
const global_cfg = cfg{port: 8080}
const shared_slot = slot64{v: 0}

fn main() {
    src := [102, 111, 111, 98]
    _, _ = hex.decode(dst, src)
    _ = json.unmarshal(src, &global_cfg)
    _ = json.compact(dst, src)
    json.html_escape(dst, src)
    _ = json.indent(dst, src, "", "  ")
    atomic.store_int64(&shared_slot.v, 42)
    _ = atomic.add_int64(&shared_slot.v, 1)
    _ = atomic.compare_and_swap_int64(&shared_slot.v, 0, 1)
    _, _ = fmt.sscan("42", &global_cfg)
    _, _ = fmt.sscanf("42", "%d", &shared_slot.v)
}
"#,
    );
    assert_eq!(r.verdict, Verdict::Reject, "got {:?}", r.diagnostics);
    assert!(
        codes(&r).iter().any(|c| c == "HYB-E3006"),
        "expected HYB-E3006, got {:?}",
        r.diagnostics
    );
}

#[test]
fn unproven_atomic_value_cannot_cross_a_task_boundary() {
    let r = check(
        r#"import "sync/atomic"
fn leak(v int) {}
fn main() {
    v := atomic.value(1)
    task leak(v)
}
"#,
    );
    assert_eq!(r.verdict, Verdict::Reject, "got {:?}", r.diagnostics);
    assert!(
        codes(&r).iter().any(|c| c == "HYB-E3005"),
        "expected HYB-E3005, got {:?}",
        r.diagnostics
    );
}
