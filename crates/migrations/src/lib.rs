pub use sea_orm_migration::MigratorTrait;
use sea_orm_migration::prelude::*;
mod m001_users;
mod m002_credentials;
mod m003_vms;
mod m004_networks;
mod m005_user_status;
mod m006_activity;
mod m007_secrets;
mod m008_kernel_aliases;
mod m009_file_assets;
mod m010_snapshots;
mod m011_upstream_proxies;
mod m012_egress_policies;
mod m013_vm_egress_policies;
pub struct Migrator;
#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m001_users::Migration),
            Box::new(m002_credentials::Migration),
            Box::new(m003_vms::Migration),
            Box::new(m004_networks::Migration),
            Box::new(m005_user_status::Migration),
            Box::new(m006_activity::Migration),
            Box::new(m007_secrets::Migration),
            Box::new(m008_kernel_aliases::Migration),
            Box::new(m009_file_assets::Migration),
            Box::new(m010_snapshots::Migration),
            Box::new(m011_upstream_proxies::Migration),
            Box::new(m012_egress_policies::Migration),
            Box::new(m013_vm_egress_policies::Migration),
        ]
    }
}
