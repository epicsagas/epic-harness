use glib::variant::ToVariant;

// Run with --release: the original immutable C out-pointer becomes UB when
// optimized. Exercise both Iterator and DoubleEndedIterator entry points.
#[test]
fn issue_138_variant_string_iterator_updates_its_mutable_out_pointer() {
    let variant = ["alpha", "beta", "gamma"].to_variant();
    let mut forward = variant.array_iter_str().unwrap();
    assert_eq!(forward.next(), Some("alpha"));
    assert_eq!(forward.nth(1), Some("gamma"));
    assert_eq!(forward.next(), None);

    let mut backward = variant.array_iter_str().unwrap();
    assert_eq!(backward.next_back(), Some("gamma"));
    assert_eq!(backward.nth_back(1), Some("alpha"));
    assert_eq!(backward.next_back(), None);

    assert_eq!(
        variant.array_iter_str().unwrap().collect::<Vec<_>>(),
        ["alpha", "beta", "gamma"]
    );
    assert_eq!(
        variant.array_iter_str().unwrap().rev().collect::<Vec<_>>(),
        ["gamma", "beta", "alpha"]
    );
}
