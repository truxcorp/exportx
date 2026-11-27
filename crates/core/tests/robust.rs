//! Review of PR #22 (round 1): reproductions at the core's level.

use std::fs;
use std::io;
use std::path::Path;

use exportpro_core::apply::{finish_copy, lock_shoots_with, plan_locked_with};
use exportpro_core::erase::mount_source;
use exportpro_core::{DateChoice, Layout, Request};

fn card(dir: &Path, names: &[&str]) -> Vec<exportpro_core::Group> {
    let src = dir.join("card");
    fs::create_dir_all(&src).unwrap();
    for n in names {
        fs::write(src.join(n), n).unwrap();
    }
    exportpro_core::scan(&src).unwrap()
}

fn request(dir: &Path) -> Request {
    Request {
        dest: dir.join("photos"),
        shoot: "s".into(),
        prefix: "txo".into(),
        date: DateChoice::NoDate,
        layout: Layout::default(),
    }
}

#[test]
fn numbers_are_read_again_once_the_shoot_is_locked() {
    let dir = tempfile::tempdir().unwrap();
    let groups = card(dir.path(), &["a.jpg"]);
    let req = request(dir.path());

    // Another copy finishes between the first plan and the lock: it wrote txo_0001.
    let (plan, _locks) = plan_locked_with(&groups, &req, || {
        let jpg = dir.path().join("photos/s/p/o/jpg");
        fs::create_dir_all(&jpg).unwrap();
        fs::write(jpg.join("txo_0001.jpg"), "other copy").unwrap();
    })
    .unwrap();

    assert_eq!(plan.entries[0].dest, Path::new("s/p/o/jpg/txo_0002.jpg"));
}

#[test]
fn a_file_system_without_locks_still_copies() {
    let dir = tempfile::tempdir().unwrap();
    let groups = card(dir.path(), &["a.jpg"]);
    let req = request(dir.path());
    let plan = exportpro_core::plan(&groups, &req).unwrap();

    let locks = lock_shoots_with(&plan, &req, |_| {
        Err(fs::TryLockError::Error(io::Error::from(
            io::ErrorKind::Unsupported,
        )))
    });

    assert!(locks.is_ok(), "{:?}", locks.err());
}

#[test]
fn a_copy_whose_part_cannot_be_removed_is_still_a_copy() {
    let dir = tempfile::tempdir().unwrap();
    let part = dir.path().join("txo_0001.jpg.part");
    let dst = dir.path().join("txo_0001.jpg");
    fs::write(&part, "photo").unwrap();

    let done = finish_copy(&part, &dst, |_| {
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    });

    assert!(done.is_ok(), "{:?}", done.err());
    assert_eq!(fs::read_to_string(&dst).unwrap(), "photo");
}

#[test]
fn the_disk_behind_a_btrfs_subvolume_is_found_in_mountinfo() {
    let mountinfo = "\
29 1 0:26 /root / rw,relatime shared:1 - btrfs /dev/nvme0n1p3 rw,compress=zstd:1,subvol=/root
30 29 0:27 /home /home rw,relatime shared:2 - btrfs /dev/nvme0n1p3 rw,subvol=/home
31 29 8:17 / /media/card rw,relatime shared:3 - vfat /dev/sdb1 rw
";
    assert_eq!(
        mount_source(mountinfo, 0, 26).as_deref(),
        Some("/dev/nvme0n1p3")
    );
    assert_eq!(
        mount_source(mountinfo, 0, 27).as_deref(),
        Some("/dev/nvme0n1p3")
    );
    assert_eq!(mount_source(mountinfo, 8, 17).as_deref(), Some("/dev/sdb1"));
    assert_eq!(mount_source(mountinfo, 9, 9), None);
}

// Review of PR #22 (round 2).

#[test]
fn a_lock_file_replaced_while_locking_is_not_trusted() {
    let dir = tempfile::tempdir().unwrap();
    let groups = card(dir.path(), &["a.jpg"]);
    let req = request(dir.path());
    let plan = exportpro_core::plan(&groups, &req).unwrap();
    let lock_path = dir.path().join("photos/s/.exportx.lock");
    let theirs = std::cell::RefCell::new(None);
    let calls = std::cell::Cell::new(0);

    // While we lock the file we opened, another copy deletes it and locks a new one.
    let locks = lock_shoots_with(&plan, &req, |ours| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            fs::remove_file(&lock_path).unwrap();
            let other = fs::File::create(&lock_path).unwrap();
            other.try_lock().unwrap();
            *theirs.borrow_mut() = Some(other);
        }
        ours.try_lock()
    });

    assert!(
        matches!(locks, Err(exportpro_core::PlanError::Busy(_))),
        "{:?}",
        locks.map(|_| ())
    );
}

#[test]
fn a_stale_part_linked_to_a_finished_photo_never_overwrites_it() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.jpg");
    fs::write(&src, "new photo").unwrap();
    let finished = dir.path().join("moved-elsewhere.jpg");
    fs::write(&finished, "finished photo").unwrap();
    let dst = dir.path().join("txo_0001.jpg");
    // A crash between the link and the unlink left the .part as a second name.
    fs::hard_link(&finished, dir.path().join("txo_0001.jpg.part")).unwrap();

    exportpro_core::apply::copy_verified(&src, &dst).unwrap();

    assert_eq!(fs::read_to_string(&finished).unwrap(), "finished photo");
    assert_eq!(fs::read_to_string(&dst).unwrap(), "new photo");
}

#[test]
fn a_copy_refused_after_locking_leaves_no_empty_shoot_folder() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("card");
    fs::create_dir_all(&src).unwrap();
    for (name, day) in [("a.jpg", 0u64), ("b.jpg", 2)] {
        let p = src.join(name);
        fs::write(&p, name).unwrap();
        let t = std::time::SystemTime::UNIX_EPOCH
            + std::time::Duration::from_secs(1_709_596_800 + day * 86_400 + 3600);
        fs::File::options()
            .write(true)
            .open(&p)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(t))
            .unwrap();
    }
    let groups = exportpro_core::scan(&src).unwrap();
    let req = Request {
        date: DateChoice::EachDay,
        ..request(dir.path())
    };
    // The second day's shoot is being copied by someone else.
    let busy = dir.path().join("photos/2024/03/07/s");
    fs::create_dir_all(&busy).unwrap();
    let lock = fs::File::create(busy.join(".exportx.lock")).unwrap();
    lock.try_lock().unwrap();

    let result = exportpro_core::apply::plan_locked(&groups, &req);

    assert!(matches!(result, Err(exportpro_core::PlanError::Busy(_))));
    assert!(
        !dir.path().join("photos/2024/03/05/s").exists(),
        "empty folder left behind"
    );
}

#[test]
fn the_disk_behind_a_folder_is_found_by_mount_point() {
    use exportpro_core::erase::mount_source_for;
    let mountinfo = "\
29 1 0:26 /root / rw,relatime shared:1 - btrfs /dev/nvme0n1p3 rw,subvol=/root
30 29 0:27 /home /home rw,relatime shared:2 - btrfs /dev/nvme0n1p3 rw,subvol=/home
31 29 8:17 / /media/my\\040card rw,relatime shared:3 - vfat /dev/sdb1 rw
32 29 0:40 / /tmp rw shared:4 - tmpfs tmpfs rw
";
    let disk = |p: &str| mount_source_for(mountinfo, Path::new(p));
    assert_eq!(disk("/home/me/Photos").as_deref(), Some("/dev/nvme0n1p3"));
    assert_eq!(disk("/var/photos").as_deref(), Some("/dev/nvme0n1p3"));
    assert_eq!(disk("/media/my card/DCIM").as_deref(), Some("/dev/sdb1"));
    assert_eq!(disk("/tmp/x").as_deref(), Some("tmpfs"));
}
