// 筛选 + 抽签 + 历史。对应 tools/roll_squad.py。
//
// 语义必须和 Python 版逐条对齐（配额是**下限**不是固定值、余席随机填、池子不够时
// 先放弃历史规避再报错）。随机数算法不同 → 同种子抽出的队不会和 Python 版一样，
// 这是已知且可接受的差异。

use crate::model::{History, HistoryEntry, HistoryMember, Operator};
use crate::paths;
use crate::roster::{PROFESSION_ORDER, UNKNOWN_PROF};
use rand::seq::{IndexedRandom, SliceRandom};   // 0.9 起 choose_* 在 IndexedRandom，shuffle 在 SliceRandom
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// 默认配额：**八个职业各一名**（用户定的）。
/// 12 人队 = 8 个保底席位 + 4 个随机席；某个职业池子里没人时按老规矩告警并随机补。
pub fn default_floor() -> HashMap<String, u32> {
    PROFESSION_ORDER.iter().map(|p| (p.to_string(), 1u32)).collect()
}

#[derive(Debug, Clone, Default)]
pub struct RollReq {
    pub tier: u32,
    pub mode: String,
    pub quota: Option<HashMap<String, u32>>,
    pub n: usize,
    pub seed: Option<u64>,
    pub avoid_last: u32,
    pub exclude: Vec<String>,
    pub rarities: Option<Vec<u32>>,
    pub min_elite: Option<u32>,
    pub min_level: Option<u32>,
    pub max_level: Option<u32>,
    pub min_rarity: Option<u32>,
    pub slots: Option<Vec<Option<String>>>,
}

pub struct RollOutcome {
    pub squad: Vec<Operator>,
    pub seed: u64,
    pub warnings: Vec<String>,
    pub roster_n: usize,
    pub after_tier: usize,
    pub after_filter: usize,
}

// ---------------------------------------------------------------- 筛选

#[allow(clippy::too_many_arguments)]
pub fn filter_pool(
    pool: &[Operator],
    min_elite: u32,
    min_level: u32,
    max_level: Option<u32>,
    min_rarity: u32,
    rarities: Option<&[u32]>,
    excluded: &HashSet<String>,
) -> Vec<Operator> {
    pool.iter()
        .filter(|o| {
            if o.elite < min_elite || o.level < min_level || o.rarity < min_rarity {
                return false;
            }
            if let Some(rs) = rarities {
                if !rs.is_empty() && !rs.contains(&o.rarity) {
                    return false;
                }
            }
            if let Some(mx) = max_level {
                if o.level > mx {
                    return false;
                }
            }
            !excluded.contains(&o.id) && !excluded.contains(&o.name)
        })
        .cloned()
        .collect()
}

// ---------------------------------------------------------------- 抽签

pub fn compose(
    pool: &[Operator],
    n: usize,
    mode: &str,
    quota: Option<&HashMap<String, u32>>,
    slots_in: Option<Vec<Option<String>>>,
    rng: &mut StdRng,
) -> Result<(Vec<Operator>, Vec<String>), String> {
    let mut warnings: Vec<String> = Vec::new();

    let by_id: HashMap<&str, &Operator> = pool.iter().map(|o| (o.id.as_str(), o)).collect();
    let mut taken: HashSet<String> = HashSet::new();
    let mut resolved: Vec<Option<Operator>> = vec![None; n];

    if let Some(slots) = slots_in {
        if slots.len() != n {
            return Err(format!("slots 长度 {} 与人数 {} 对不上", slots.len(), n));
        }
        for (i, sid) in slots.into_iter().enumerate() {
            let Some(id) = sid else { continue };
            match by_id.get(id.as_str()) {
                Some(op) => {
                    resolved[i] = Some((*op).clone());
                    taken.insert(id);
                }
                None => warnings.push(format!("锁定的干员不在当前池子里，已忽略：{id}")),
            }
        }
    }

    let empty = resolved.iter().filter(|s| s.is_none()).count();
    let avail: Vec<Operator> = pool.iter().filter(|o| !taken.contains(&o.id)).cloned().collect();
    if avail.len() < empty {
        return Err(format!(
            "池子里只剩 {} 人，凑不满 {} 个空位 —— 放宽档位或减少排除名单",
            avail.len(),
            empty
        ));
    }

    let mut picked: Vec<Operator> = Vec::new();
    match mode {
        "pure" => {
            picked = avail.choose_multiple(rng, empty).cloned().collect();
        }
        "floor" | "quota" => {
            let target: HashMap<String, u32> = match quota {
                Some(q) => q.clone(),
                None => default_floor(),
            };
            let total: u32 = target.values().sum();
            if total as usize > empty {
                warnings.push(format!("配额共 {total} 席 > 空位 {empty} 席，超出部分已忽略"));
            }
            let mut kept: HashMap<String, usize> = HashMap::new();
            for s in resolved.iter().flatten() {
                *kept.entry(s.profession.clone()).or_default() += 1;
            }
            let mut used = taken.clone();
            let mut left = empty;
            for prof in PROFESSION_ORDER {
                if left == 0 {
                    break;
                }
                let want0 = target.get(prof).copied().unwrap_or(0) as usize;
                let already = kept.get(prof).copied().unwrap_or(0);
                let mut want = want0.saturating_sub(already);
                if want == 0 {
                    continue;
                }
                want = want.min(left);
                // 先按本职抽；不够再让能转职的顶上（阿米娅：术师/近卫/医疗）
                let natural: Vec<&Operator> = avail
                    .iter()
                    .filter(|o| o.profession == prof && !used.contains(&o.id))
                    .collect();
                let alts: Vec<&Operator> = avail
                    .iter()
                    .filter(|o| {
                        o.profession != prof
                            && o.alt_professions.iter().any(|a| a == prof)
                            && !used.contains(&o.id)
                    })
                    .collect();
                let total_cand = natural.len() + alts.len();
                if total_cand < want {
                    warnings.push(format!(
                        "{prof} 池子里只有 {total_cand} 人，凑不满 {want} 席（缺口随机补）"
                    ));
                }
                let mut got = 0;
                for op in natural.choose_multiple(rng, want.min(natural.len())) {
                    picked.push((**op).clone()); // op 是 &&Operator，要解两层
                    used.insert(op.id.clone());
                    got += 1;
                }
                let remain = want.saturating_sub(got);
                if remain > 0 {
                    for op in alts.choose_multiple(rng, remain.min(alts.len())) {
                        let mut o = (**op).clone();
                        o.profession = prof.to_string(); // 本局按这个职业出战
                        o.converted = true;
                        used.insert(o.id.clone());
                        picked.push(o);
                        got += 1;
                    }
                }
                left -= got;
            }
            let rest: Vec<&Operator> = avail.iter().filter(|o| !used.contains(&o.id)).collect();
            let fill = left.min(rest.len());
            for op in rest.choose_multiple(rng, fill) {
                picked.push((**op).clone());
            }
        }
        other => return Err(format!("未知模式：{other}（可选：pure / floor / quota）")),
    }

    picked.shuffle(rng); // 配额模式是按职业顺序抽的，打散一下
    let mut it = picked.into_iter();
    let mut squad: Vec<Operator> = Vec::with_capacity(n);
    for slot in resolved {
        match slot {
            Some(op) => squad.push(op),
            None => {
                if let Some(op) = it.next() {
                    squad.push(op);
                }
            }
        }
    }
    Ok((squad, warnings))
}

// ---------------------------------------------------------------- 历史

pub fn load_history() -> History {
    std::fs::read_to_string(paths::history_path())
        .ok()
        .and_then(|t| serde_json::from_str::<History>(&t).ok())
        .unwrap_or_default()
}

pub fn append_history(entry: HistoryEntry) -> Result<PathBuf, String> {
    let mut h = load_history();
    h.rolls.push(entry);
    let _ = paths::ensure_dir(&paths::data_dir());
    let path = paths::history_path();
    let text = serde_json::to_string_pretty(&h).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("写历史失败：{e}"))?;
    Ok(path)
}

/// 最近 avoid_last 轮抽到过的干员 id
pub fn recent_ids(avoid_last: u32) -> HashSet<String> {
    if avoid_last == 0 {
        return HashSet::new();
    }
    let h = load_history();
    let start = h.rolls.len().saturating_sub(avoid_last as usize);
    h.rolls[start..]
        .iter()
        .flat_map(|r| r.members.iter().map(|m| m.id.clone()))
        .collect()
}

// ---------------------------------------------------------------- 主流程

pub fn build_and_roll(roster: &[Operator], req: &RollReq) -> Result<RollOutcome, String> {
    let n = req.n.max(1);

    // 模式归一：老的 "floor" 并入 "quota"（两者本就是同一机制）
    let mode = match req.mode.as_str() {
        "pure" => "pure",
        _ => "quota",
    };

    // 配额校验
    if let Some(q) = &req.quota {
        for prof in q.keys() {
            if !PROFESSION_ORDER.contains(&prof.as_str()) {
                return Err(format!("没有这个职业：{prof}（可选：{}）", PROFESSION_ORDER.join("/")));
            }
        }
        let total: u32 = q.values().sum();
        if mode == "quota" && total as usize > n {
            return Err(format!("配额共 {total} 席 > 人数 {n} —— 减配额或加人数"));
        }
    }

    let min_elite = req.min_elite.unwrap_or(0);
    let min_level = req.min_level.unwrap_or(0);
    let after_tier = crate::roster::apply_tier(roster, req.tier, min_elite, min_level);

    let mut excluded: HashSet<String> = req.exclude.iter().cloned().collect();
    let avoid = recent_ids(req.avoid_last);
    let mut warnings: Vec<String> = Vec::new();

    let with_avoid: HashSet<String> = excluded.union(&avoid).cloned().collect();
    let mut pool = filter_pool(
        &after_tier,
        min_elite,
        min_level,
        req.max_level,
        req.min_rarity.unwrap_or(0),
        req.rarities.as_deref(),
        &with_avoid,
    );

    if !avoid.is_empty() && pool.len() < n {
        let relaxed = filter_pool(
            &after_tier,
            min_elite,
            min_level,
            req.max_level,
            req.min_rarity.unwrap_or(0),
            req.rarities.as_deref(),
            &excluded,
        );
        if relaxed.len() >= n {
            warnings.push(format!(
                "排除最近 {} 轮后只剩 {} 人、不够 {n} 席，本轮已忽略历史规避",
                req.avoid_last,
                pool.len()
            ));
            pool = relaxed;
        }
    }
    if pool.is_empty() {
        excluded.clear();
    }
    if pool.len() < n {
        return Err(format!(
            "门槛过完只剩 {} 人，不足 {n} 席 —— 换低一点的档位，或减少排除名单",
            pool.len()
        ));
    }

    let seed = req.seed.unwrap_or_else(|| rand::rng().random_range(1..2_147_483_648u64));
    let mut rng = StdRng::seed_from_u64(seed);
    let (squad, mut cw) = compose(&pool, n, mode, req.quota.as_ref(), req.slots.clone(), &mut rng)?;
    warnings.append(&mut cw);

    let entry = HistoryEntry {
        time: now_iso(),
        seed,
        mode: mode.to_string(),
        tier: req.tier,
        n: n as u32,
        quota: req.quota.as_ref().map(|q| serde_json::to_value(q).unwrap_or(serde_json::Value::Null)),
        pool: serde_json::json!({
            "roster": roster.len(), "after_tier": after_tier.len(), "after_filter": pool.len()
        }),
        members: squad
            .iter()
            .map(|o| HistoryMember {
                id: o.id.clone(),
                name: o.name.clone(),
                profession: o.profession.clone(),
                rarity: o.rarity,
                elite: o.elite,
                level: o.level,
            })
            .collect(),
    };
    let _ = append_history(entry);

    Ok(RollOutcome {
        squad,
        seed,
        warnings,
        roster_n: roster.len(),
        after_tier: after_tier.len(),
        after_filter: pool.len(),
    })
}

fn now_iso() -> String {
    // 只到秒的本地时间近似（用系统时间戳换算，不做时区库依赖）
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 历史里存的那条记录 → 前端要的形状（成员只给名字）
pub fn history_for_ui(history: &History, limit: usize) -> Vec<serde_json::Value> {
    let start = history.rolls.len().saturating_sub(limit);
    history.rolls[start..]
        .iter()
        .map(|r| {
            serde_json::json!({
                "time": r.time,
                "seed": r.seed,
                "mode": r.mode,
                "tier": r.tier,
                "n": r.n,
                "quota": r.quota,
                "names": r.members.iter().map(|m| m.name.clone()).collect::<Vec<_>>(),
            })
        })
        .collect()
}

/// 队形统计（前端概要行要显示）
pub fn composition(squad: &[Operator]) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    for p in PROFESSION_ORDER {
        let n = squad.iter().filter(|o| o.profession == p).count();
        if n > 0 {
            m.insert(p.to_string(), serde_json::json!(n));
        }
    }
    let unk = squad.iter().filter(|o| o.profession == UNKNOWN_PROF).count();
    if unk > 0 {
        m.insert(UNKNOWN_PROF.to_string(), serde_json::json!(unk));
    }
    serde_json::Value::Object(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Operator;

    fn op(id: &str, name: &str, prof: &str, alt: Vec<&str>) -> Operator {
        Operator {
            id: id.into(), name: name.into(), rarity: 5, elite: 2, level: 80, potential: 1,
            own: true, profession: prof.into(), sub_profession: None, skill_level: None,
            module: None, alt_professions: alt.into_iter().map(|s| s.to_string()).collect(),
            converted: false,
        }
    }

    /// 阿米娅：本职术师，能顶近卫/医疗。这里池子里没有医疗，她应当被顶上去当医疗。
    #[test]
    fn amiya_fills_missing_profession() {
        let pool = vec![
            op("char_002_amiya", "阿米娅", "术师", vec!["近卫", "医疗"]),
            op("x1", "某先锋", "先锋", vec![]),
        ];
        let mut q = HashMap::new();
        q.insert("先锋".to_string(), 1u32);
        q.insert("医疗".to_string(), 1u32);
        let mut rng = StdRng::seed_from_u64(7);
        let (squad, _w) = compose(&pool, 2, "quota", Some(&q), None, &mut rng).unwrap();
        let amiya = squad.iter().find(|o| o.name == "阿米娅").expect("她应当在场");
        assert_eq!(amiya.profession, "医疗", "没有医疗时她该顶上医疗");
        assert!(amiya.converted, "应当标记为转职");
        assert_eq!(squad.iter().filter(|o| o.profession == "医疗").count(), 1);
    }

    /// 有真医疗时，不该抢她的本职 -> 她仍按术师出场，且不标转职。
    #[test]
    fn amiya_stays_caster_when_not_needed() {
        let pool = vec![
            op("char_002_amiya", "阿米娅", "术师", vec!["近卫", "医疗"]),
            op("x2", "某医疗", "医疗", vec![]),
        ];
        let mut q = HashMap::new();
        q.insert("医疗".to_string(), 1u32);
        let mut rng = StdRng::seed_from_u64(3);
        let (squad, _w) = compose(&pool, 2, "quota", Some(&q), None, &mut rng).unwrap();
        let amiya = squad.iter().find(|o| o.name == "阿米娅").unwrap();
        assert_eq!(amiya.profession, "术师");
        assert!(!amiya.converted);
    }

    /// 锁定槽位要原样保留，其余照抽。
    #[test]
    fn locked_slots_are_kept() {
        let pool = vec![
            op("a", "甲", "先锋", vec![]), op("b", "乙", "近卫", vec![]),
            op("c", "丙", "重装", vec![]), op("d", "丁", "狙击", vec![]),
        ];
        let mut rng = StdRng::seed_from_u64(11);
        let slots = Some(vec![Some("b".to_string()), None, None]);
        let (squad, _w) = compose(&pool, 3, "pure", None, slots, &mut rng).unwrap();
        assert_eq!(squad[0].id, "b", "锁定的必须留在原槽位");
        let ids: std::collections::HashSet<_> = squad.iter().map(|o| o.id.clone()).collect();
        assert_eq!(ids.len(), 3, "队内不能重复");
    }

    /// 池子凑不满要报错，不许静默给半支队。
    #[test]
    fn too_small_pool_errors() {
        let pool = vec![op("a", "甲", "先锋", vec![])];
        let mut rng = StdRng::seed_from_u64(1);
        let err = compose(&pool, 3, "pure", None, None, &mut rng).unwrap_err();
        assert!(err.contains("凑不满"), "错误信息要能看懂：{err}");
    }
}
