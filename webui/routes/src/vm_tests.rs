use super::*;
#[test]
fn vm_paths_preserve_edit_create_and_every_tab() {
    assert_eq!(parse_vm_path(""), VmPath::Inventory);
    assert_eq!(parse_vm_path("new"), VmPath::Create);
    assert_eq!(parse_vm_path("worker/edit"), VmPath::Edit("worker".into()));
    assert_eq!(
        parse_vm_path("worker"),
        VmPath::Detail {
            id: "worker".into(),
            tab: VmTab::Overview
        }
    );
    assert_eq!(VmTab::from_slug("security"), VmTab::Isolation);
    for tab in VmTab::ALL {
        assert_eq!(
            parse_vm_path(&format!("worker/{}", tab.slug())),
            VmPath::Detail {
                id: "worker".into(),
                tab
            }
        );
    }
    assert_eq!(
        parse_vm_path("worker/unknown"),
        VmPath::Detail {
            id: "worker".into(),
            tab: VmTab::Overview
        }
    );
    assert_eq!(parse_vm_path("worker/serial/extra"), VmPath::Inventory);
}
