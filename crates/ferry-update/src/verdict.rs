//! 判断一份校验过的清单对本机意味着什么。

use semver::Version;

use crate::manifest::Release;

/// 判断的结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// 有比本机新的版本。
    Newer,
    /// 本机已是最新。
    UpToDate,
    /// 清单已经过期：可能有人一直给出旧清单。不能说“已是最新”。
    Expired,
    /// 清单比以前见过的旧：可能有人拿旧清单冒充。不能说“已是最新”。
    Rollback,
}

/// `now`、`seen`（以前见过的最新一份清单的发布时间）都是 Unix 秒。
pub fn judge(release: &Release, current: &Version, now: i64, seen: Option<i64>) -> Verdict {
    if seen.is_some_and(|seen| release.published < seen) {
        Verdict::Rollback
    } else if now >= release.expires {
        Verdict::Expired
    } else if release.version > *current {
        Verdict::Newer
    } else {
        Verdict::UpToDate
    }
}
