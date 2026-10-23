//! Performance benchmarks for RBAC operations

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use llm_config_rbac::{permissions::*, PolicyEnforcer, Role, RoleAssignment};

fn bench_role_assignment(c: &mut Criterion) {
    let mut group = c.benchmark_group("role_assignment");

    for num_users in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*num_users as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_users),
            num_users,
            |b, &num_users| {
                b.iter_batched(
                    PolicyEnforcer::new,
                    |mut enforcer| {
                        for i in 0..num_users {
                            let role = match i % 4 {
                                0 => Role::Admin,
                                1 => Role::Editor,
                                2 => Role::Viewer,
                                _ => Role::Auditor,
                            };
                            enforcer.assign_role(RoleAssignment::new(format!("user{}", i), role));
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn bench_permission_check(c: &mut Criterion) {
    let mut enforcer = PolicyEnforcer::new();

    // Setup various users with different roles
    enforcer.assign_role(RoleAssignment::new("admin", Role::Admin));
    enforcer.assign_role(RoleAssignment::new("editor", Role::Editor));
    enforcer.assign_role(RoleAssignment::new("viewer", Role::Viewer));
    enforcer.assign_role(RoleAssignment::new("auditor", Role::Auditor));

    let mut group = c.benchmark_group("permission_check");

    group.bench_function("admin_can_delete", |b| {
        b.iter(|| {
            enforcer
                .check_permission(
                    black_box("admin"),
                    black_box(&Resource::Config),
                    black_box(&Action::Delete),
                    None,
                )
                .unwrap()
        });
    });

    group.bench_function("editor_can_update", |b| {
        b.iter(|| {
            enforcer
                .check_permission(
                    black_box("editor"),
                    black_box(&Resource::Config),
                    black_box(&Action::Update),
                    None,
                )
                .unwrap()
        });
    });

    group.bench_function("viewer_can_read", |b| {
        b.iter(|| {
            enforcer
                .check_permission(
                    black_box("viewer"),
                    black_box(&Resource::Config),
                    black_box(&Action::Read),
                    None,
                )
                .unwrap()
        });
    });

    group.bench_function("viewer_cannot_update", |b| {
        b.iter(|| {
            let _ = enforcer.check_permission(
                black_box("viewer"),
                black_box(&Resource::Config),
                black_box(&Action::Update),
                None,
            );
        });
    });

    group.finish();
}

fn bench_namespace_permission_check(c: &mut Criterion) {
    let mut enforcer = PolicyEnforcer::new();

    enforcer.assign_role(RoleAssignment::new("user1", Role::Editor));

    c.bench_function("namespace_permission_check", |b| {
        b.iter(|| {
            enforcer
                .check_permission(
                    black_box("user1"),
                    black_box(&Resource::Config),
                    black_box(&Action::Update),
                    Some(black_box("production/*")),
                )
                .unwrap()
        });
    });
}

fn bench_mixed_permission_checks(c: &mut Criterion) {
    let mut enforcer = PolicyEnforcer::new();

    // Setup 100 users with different roles
    for i in 0..100 {
        let role = match i % 4 {
            0 => Role::Admin,
            1 => Role::Editor,
            2 => Role::Viewer,
            _ => Role::Auditor,
        };
        enforcer.assign_role(RoleAssignment::new(format!("user{}", i), role));
    }

    c.bench_function("mixed_permission_checks", |b| {
        b.iter(|| {
            for i in 0..100 {
                let user = format!("user{}", i);
                let resource = match i % 3 {
                    0 => Resource::Config,
                    1 => Resource::Secret,
                    _ => Resource::System,
                };
                let action = match i % 4 {
                    0 => Action::Read,
                    1 => Action::Create,
                    2 => Action::Update,
                    _ => Action::Delete,
                };
                let _ = enforcer.check_permission(&user, &resource, &action, None);
            }
        });
    });
}

fn bench_role_revocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("role_revocation");

    for num_users in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*num_users as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_users),
            num_users,
            |b, &num_users| {
                b.iter_batched(
                    || {
                        let mut enforcer = PolicyEnforcer::new();
                        for i in 0..num_users {
                            enforcer.assign_role(RoleAssignment::new(
                                format!("user{}", i),
                                Role::Editor,
                            ));
                        }
                        enforcer
                    },
                    |mut enforcer| {
                        for i in 0..num_users {
                            let _ = enforcer.revoke_role(&format!("user{}", i), 0);
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn bench_get_user_roles(c: &mut Criterion) {
    let mut enforcer = PolicyEnforcer::new();

    // Setup users
    enforcer.assign_role(RoleAssignment::new("admin", Role::Admin));
    enforcer.assign_role(RoleAssignment::new("editor", Role::Editor));
    enforcer.assign_role(RoleAssignment::new("viewer", Role::Viewer));

    let mut group = c.benchmark_group("get_user_roles");

    group.bench_function("admin_roles", |b| {
        b.iter(|| enforcer.get_user_roles(black_box("admin")));
    });

    group.bench_function("viewer_roles", |b| {
        b.iter(|| enforcer.get_user_roles(black_box("viewer")));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_role_assignment,
    bench_permission_check,
    bench_namespace_permission_check,
    bench_mixed_permission_checks,
    bench_role_revocation,
    bench_get_user_roles
);
criterion_main!(benches);
