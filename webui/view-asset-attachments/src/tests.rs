use super::*;
use serde_json::json;

#[test]
fn forms_preserve_identity_paths_and_unix_permissions() {
    let spec = json!({"attachments":[{"asset_id":"00000000-0000-0000-0000-000000000001", "destination":"/workspace/config.json", "uid":1000,"gid":1001,"mode":493}]});
    let forms = from_spec(&spec);
    assert_eq!(forms[0].mode, "0755");
    assert_eq!(
        serde_json::to_value(attachments(&forms).unwrap()).unwrap(),
        spec["attachments"]
    );
}
#[test]
fn invalid_attachment_fields_never_silently_fall_back() {
    let mut form = AttachmentForm {
        asset_id: "00000000-0000-0000-0000-000000000001".into(),
        destination: "/workspace/config".into(),
        ..Default::default()
    };
    assert!(form.attachment().is_ok());
    form.mode = "0899".into();
    assert!(form.attachment().is_err());
    form.mode = "0644".into();
    form.uid = "-1".into();
    assert!(form.attachment().is_err());
    form.uid = "0".into();
    form.destination = "/workspace/../etc/config".into();
    assert!(form.attachment().is_err());
}
