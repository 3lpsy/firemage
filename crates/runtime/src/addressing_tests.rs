use super::*;

#[test]
fn suggestion_skips_gateway_reservations_and_wraps_inside_small_subnets() {
    let net: NetworkSpec = serde_json::from_value(
        serde_json::json!({"name":"review","subnet":"10.77.1.0/29","gateway":"10.77.1.1"}),
    )
    .unwrap();
    let mut reserved = BTreeSet::from([
        "10.77.1.2".parse::<Ipv4Addr>().unwrap(),
        "10.77.1.3".parse::<Ipv4Addr>().unwrap(),
    ]);
    assert_eq!(
        suggest_address(&net, &reserved).unwrap().to_string(),
        "10.77.1.4"
    );
    reserved.extend([
        "10.77.1.4".parse::<Ipv4Addr>().unwrap(),
        "10.77.1.5".parse::<Ipv4Addr>().unwrap(),
        "10.77.1.6".parse::<Ipv4Addr>().unwrap(),
    ]);
    assert!(suggest_address(&net, &reserved).is_none());
    let net: NetworkSpec = serde_json::from_value(
        serde_json::json!({"name":"review","subnet":"10.77.1.0/30","gateway":"10.77.1.2"}),
    )
    .unwrap();
    assert_eq!(
        suggest_address(&net, &BTreeSet::new()).unwrap().to_string(),
        "10.77.1.1"
    );
}
