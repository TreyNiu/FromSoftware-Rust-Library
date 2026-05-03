// 正式随机池来自 1.16.1的regulation.bin：
// 1. Name 非空；
// 2. wepmotionCategory != 0；
// 3. Name 不包含 NPC；
// 4. 只保留 ID % 10000 == 0 的基础武器行。
//
// 第 4 条很重要：Heavy/Keen/Cold 这类质变行和强化等级行不能直接作为“基础武器”放进池子，
// 否则后面再叠加质变 offset 和强化等级时会得到错误的 EquipParamWeapon id。
pub const DEFAULT_WEPMOTION_CATEGORIES: &[u16] = &[
    20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43,
    44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 55, 56, 57, 58, 60, 61, 62,
];

pub fn enabled_weapon_ids(enabled_categories: &[u16]) -> Vec<u32> {
    let mut ids = Vec::new();
    for pool in WEAPON_POOLS {
        if enabled_categories.contains(&pool.wepmotion_category) {
            ids.extend_from_slice(pool.ids);
        }
    }
    ids
}

pub fn enabled_pool_summary(enabled_categories: &[u16]) -> String {
    let enabled = WEAPON_POOLS
        .iter()
        .filter(|pool| enabled_categories.contains(&pool.wepmotion_category))
        .map(|pool| {
            format!(
                "{}:{}({})",
                pool.wepmotion_category,
                pool.label,
                pool.ids.len()
            )
        })
        .collect::<Vec<_>>();
    enabled.join(", ")
}

pub struct WeaponPool {
    pub wepmotion_category: u16,
    pub label: &'static str,
    pub ids: &'static [u32],
}

pub const WEAPON_POOLS: &[WeaponPool] = &[
    WeaponPool {
        wepmotion_category: 20,
        label: "短剑 / Dagger",
        ids: WEAPON_POOL_20,
    },
    WeaponPool {
        wepmotion_category: 21,
        label: "火把 / Torch",
        ids: WEAPON_POOL_21,
    },
    WeaponPool {
        wepmotion_category: 22,
        label: "爪 / Claw",
        ids: WEAPON_POOL_22,
    },
    WeaponPool {
        wepmotion_category: 23,
        label: "直剑 / Straight Sword",
        ids: WEAPON_POOL_23,
    },
    WeaponPool {
        wepmotion_category: 24,
        label: "双头剑 / Twinblade",
        ids: WEAPON_POOL_24,
    },
    WeaponPool {
        wepmotion_category: 25,
        label: "大剑 / Greatsword",
        ids: WEAPON_POOL_25,
    },
    WeaponPool {
        wepmotion_category: 26,
        label: "特大剑 / Colossal Sword",
        ids: WEAPON_POOL_26,
    },
    WeaponPool {
        wepmotion_category: 27,
        label: "刺剑 / Thrusting Sword",
        ids: WEAPON_POOL_27,
    },
    WeaponPool {
        wepmotion_category: 28,
        label: "曲剑 / Curved Sword",
        ids: WEAPON_POOL_28,
    },
    WeaponPool {
        wepmotion_category: 29,
        label: "刀 / Katana",
        ids: WEAPON_POOL_29,
    },
    WeaponPool {
        wepmotion_category: 30,
        label: "斧 / Axe",
        ids: WEAPON_POOL_30,
    },
    WeaponPool {
        wepmotion_category: 31,
        label: "特大武器 / Colossal Weapon",
        ids: WEAPON_POOL_31,
    },
    WeaponPool {
        wepmotion_category: 32,
        label: "大斧 / Greataxe",
        ids: WEAPON_POOL_32,
    },
    WeaponPool {
        wepmotion_category: 33,
        label: "槌 / Hammer",
        ids: WEAPON_POOL_33,
    },
    WeaponPool {
        wepmotion_category: 34,
        label: "连枷 / Flail",
        ids: WEAPON_POOL_34,
    },
    WeaponPool {
        wepmotion_category: 35,
        label: "大槌 / Great Hammer",
        ids: WEAPON_POOL_35,
    },
    WeaponPool {
        wepmotion_category: 36,
        label: "矛 / Spear",
        ids: WEAPON_POOL_36,
    },
    WeaponPool {
        wepmotion_category: 37,
        label: "大矛 / Great Spear",
        ids: WEAPON_POOL_37,
    },
    WeaponPool {
        wepmotion_category: 38,
        label: "戟 / Halberd",
        ids: WEAPON_POOL_38,
    },
    WeaponPool {
        wepmotion_category: 39,
        label: "重刺剑 / Heavy Thrusting Sword",
        ids: WEAPON_POOL_39,
    },
    WeaponPool {
        wepmotion_category: 40,
        label: "大曲剑 / Curved Greatsword",
        ids: WEAPON_POOL_40,
    },
    WeaponPool {
        wepmotion_category: 41,
        label: "法杖与圣印 / Staff + Seal",
        ids: WEAPON_POOL_41,
    },
    WeaponPool {
        wepmotion_category: 42,
        label: "拳套 / Fist",
        ids: WEAPON_POOL_42,
    },
    WeaponPool {
        wepmotion_category: 43,
        label: "鞭 / Whip",
        ids: WEAPON_POOL_43,
    },
    WeaponPool {
        wepmotion_category: 44,
        label: "长弓 / Bow",
        ids: WEAPON_POOL_44,
    },
    WeaponPool {
        wepmotion_category: 45,
        label: "大弓 / Greatbow",
        ids: WEAPON_POOL_45,
    },
    WeaponPool {
        wepmotion_category: 46,
        label: "弩 / Crossbow",
        ids: WEAPON_POOL_46,
    },
    WeaponPool {
        wepmotion_category: 47,
        label: "大盾 / Greatshield",
        ids: WEAPON_POOL_47,
    },
    WeaponPool {
        wepmotion_category: 48,
        label: "小盾 / Small Shield",
        ids: WEAPON_POOL_48,
    },
    WeaponPool {
        wepmotion_category: 49,
        label: "中盾 / Medium Shield",
        ids: WEAPON_POOL_49,
    },
    WeaponPool {
        wepmotion_category: 50,
        label: "镰刀 / Reaper",
        ids: WEAPON_POOL_50,
    },
    WeaponPool {
        wepmotion_category: 51,
        label: "短弓 / Light Bow",
        ids: WEAPON_POOL_51,
    },
    WeaponPool {
        wepmotion_category: 52,
        label: "弩炮 / Ballista",
        ids: WEAPON_POOL_52,
    },
    WeaponPool {
        wepmotion_category: 53,
        label: "投掷短剑 / Throwing Blade",
        ids: WEAPON_POOL_53,
    },
    WeaponPool {
        wepmotion_category: 55,
        label: "格斗术 / Hand-to-Hand",
        ids: WEAPON_POOL_55,
    },
    WeaponPool {
        wepmotion_category: 56,
        label: "调香瓶 / Perfume Bottle",
        ids: WEAPON_POOL_56,
    },
    WeaponPool {
        wepmotion_category: 57,
        label: "突刺盾 / Thrusting Shield",
        ids: WEAPON_POOL_57,
    },
    WeaponPool {
        wepmotion_category: 58,
        label: "反手剑 / Backhand Blade",
        ids: WEAPON_POOL_58,
    },
    WeaponPool {
        wepmotion_category: 60,
        label: "轻大剑 / Light Greatsword",
        ids: WEAPON_POOL_60,
    },
    WeaponPool {
        wepmotion_category: 61,
        label: "大太刀 / Great Katana",
        ids: WEAPON_POOL_61,
    },
    WeaponPool {
        wepmotion_category: 62,
        label: "野兽爪 / Beast Claw",
        ids: WEAPON_POOL_62,
    },
];

// 20 - 短剑 / Dagger
const WEAPON_POOL_20: &[u32] = &[
    1_000_000, 1_010_000, 1_020_000, 1_030_000, 1_040_000, 1_050_000, 1_060_000, 1_070_000,
    1_080_000, 1_090_000, 1_100_000, 1_110_000, 1_130_000, 1_140_000, 1_150_000, 1_160_000,
    1_500_000, 1_510_000,
];

// 21 - 火把 / Torch
const WEAPON_POOL_21: &[u32] = &[
    24_000_000, 24_020_000, 24_040_000, 24_050_000, 24_060_000, 24_070_000, 24_500_000,
];

// 22 - 爪 / Claw
const WEAPON_POOL_22: &[u32] = &[22_000_000, 22_010_000, 22_020_000, 22_030_000, 22_500_000];

// 23 - 直剑 / Straight Sword
const WEAPON_POOL_23: &[u32] = &[
    2_000_000, 2_010_000, 2_020_000, 2_040_000, 2_050_000, 2_060_000, 2_070_000, 2_110_000,
    2_140_000, 2_150_000, 2_180_000, 2_190_000, 2_200_000, 2_210_000, 2_220_000, 2_230_000,
    2_240_000, 2_250_000, 2_260_000, 2_510_000, 2_540_000, 2_550_000, 2_560_000,
];

// 24 - 双头剑 / Twinblade
const WEAPON_POOL_24: &[u32] = &[
    10_000_000, 10_010_000, 10_030_000, 10_050_000, 10_080_000, 10_090_000, 10_500_000, 10_510_000,
];

// 25 - 大剑 / Greatsword
const WEAPON_POOL_25: &[u32] = &[
    2_090_000, 3_000_000, 3_010_000, 3_020_000, 3_030_000, 3_040_000, 3_050_000, 3_060_000,
    3_070_000, 3_080_000, 3_090_000, 3_100_000, 3_130_000, 3_140_000, 3_150_000, 3_160_000,
    3_170_000, 3_180_000, 3_190_000, 3_200_000, 3_210_000, 3_510_000, 3_520_000, 3_550_000,
];

// 26 - 特大剑 / Colossal Sword
const WEAPON_POOL_26: &[u32] = &[
    4_000_000, 4_010_000, 4_020_000, 4_030_000, 4_040_000, 4_050_000, 4_060_000, 4_070_000,
    4_080_000, 4_100_000, 4_110_000, 4_500_000, 4_520_000, 4_530_000, 4_540_000, 4_550_000,
];

// 27 - 刺剑 / Thrusting Sword
const WEAPON_POOL_27: &[u32] = &[
    2_530_000, 5_000_000, 5_010_000, 5_020_000, 5_030_000, 5_040_000, 5_050_000, 5_060_000,
];

// 28 - 曲剑 / Curved Sword
const WEAPON_POOL_28: &[u32] = &[
    2_080_000, 7_000_000, 7_010_000, 7_020_000, 7_030_000, 7_040_000, 7_050_000, 7_060_000,
    7_070_000, 7_080_000, 7_100_000, 7_110_000, 7_120_000, 7_140_000, 7_150_000, 7_500_000,
    7_510_000, 7_520_000, 7_530_000,
];

// 29 - 刀 / Katana
const WEAPON_POOL_29: &[u32] = &[
    2_520_000, 9_000_000, 9_010_000, 9_020_000, 9_030_000, 9_040_000, 9_060_000, 9_070_000,
    9_080_000, 9_500_000,
];

// 30 - 斧 / Axe
const WEAPON_POOL_30: &[u32] = &[
    14_000_000, 14_010_000, 14_020_000, 14_030_000, 14_040_000, 14_050_000, 14_060_000, 14_080_000,
    14_100_000, 14_110_000, 14_120_000, 14_140_000, 14_500_000, 14_510_000, 14_520_000, 14_540_000,
    15_010_000,
];

// 31 - 特大武器 / Colossal Weapon
const WEAPON_POOL_31: &[u32] = &[
    12_510_000, 12_530_000, 23_000_000, 23_010_000, 23_020_000, 23_030_000, 23_040_000, 23_050_000,
    23_060_000, 23_070_000, 23_080_000, 23_100_000, 23_110_000, 23_120_000, 23_130_000, 23_140_000,
    23_150_000, 23_500_000, 23_510_000, 23_520_000,
];

// 32 - 大斧 / Greataxe
const WEAPON_POOL_32: &[u32] = &[
    8_500_000, 12_140_000, 15_000_000, 15_020_000, 15_030_000, 15_040_000, 15_050_000, 15_060_000,
    15_080_000, 15_110_000, 15_120_000, 15_130_000, 15_140_000, 15_500_000, 15_510_000,
];

// 33 - 槌 / Hammer
const WEAPON_POOL_33: &[u32] = &[
    11_000_000, 11_010_000, 11_030_000, 11_040_000, 11_050_000, 11_060_000, 11_070_000, 11_080_000,
    11_090_000, 11_100_000, 11_110_000, 11_120_000, 11_130_000, 11_140_000, 11_150_000, 11_500_000,
];

// 34 - 连枷 / Flail
const WEAPON_POOL_34: &[u32] = &[
    13_000_000, 13_010_000, 13_020_000, 13_030_000, 13_040_000, 13_500_000,
];

// 35 - 大槌 / Great Hammer
const WEAPON_POOL_35: &[u32] = &[
    12_000_000, 12_010_000, 12_020_000, 12_060_000, 12_080_000, 12_130_000, 12_150_000, 12_160_000,
    12_170_000, 12_180_000, 12_190_000, 12_200_000, 12_210_000, 12_500_000, 12_520_000,
];

// 36 - 矛 / Spear
const WEAPON_POOL_36: &[u32] = &[
    16_000_000, 16_010_000, 16_020_000, 16_030_000, 16_040_000, 16_050_000, 16_060_000, 16_070_000,
    16_080_000, 16_090_000, 16_110_000, 16_120_000, 16_130_000, 16_140_000, 16_150_000, 16_160_000,
    16_500_000, 16_520_000, 16_540_000,
];

// 37 - 大矛 / Great Spear
const WEAPON_POOL_37: &[u32] = &[
    16_550_000, 17_010_000, 17_020_000, 17_030_000, 17_050_000, 17_060_000, 17_070_000, 17_500_000,
    17_510_000, 17_520_000,
];

// 38 - 戟 / Halberd
const WEAPON_POOL_38: &[u32] = &[
    18_000_000, 18_010_000, 18_020_000, 18_030_000, 18_040_000, 18_050_000, 18_060_000, 18_070_000,
    18_080_000, 18_090_000, 18_100_000, 18_110_000, 18_130_000, 18_140_000, 18_150_000, 18_160_000,
    18_500_000, 18_510_000,
];

// 39 - 重刺剑 / Heavy Thrusting Sword
const WEAPON_POOL_39: &[u32] = &[
    3_500_000, 6_000_000, 6_010_000, 6_020_000, 6_040_000, 6_500_000,
];

// 40 - 大曲剑 / Curved Greatsword
const WEAPON_POOL_40: &[u32] = &[
    8_010_000, 8_020_000, 8_030_000, 8_040_000, 8_050_000, 8_060_000, 8_070_000, 8_080_000,
    8_100_000, 8_510_000, 8_520_000,
];

// 41 - 法杖与圣印 / Staff + Seal
const WEAPON_POOL_41: &[u32] = &[
    33_000_000, 33_040_000, 33_050_000, 33_060_000, 33_090_000, 33_120_000, 33_130_000, 33_170_000,
    33_180_000, 33_190_000, 33_200_000, 33_210_000, 33_230_000, 33_240_000, 33_250_000, 33_260_000,
    33_270_000, 33_280_000, 33_510_000, 33_520_000, 34_000_000, 34_010_000, 34_020_000, 34_030_000,
    34_040_000, 34_060_000, 34_070_000, 34_080_000, 34_090_000, 34_500_000, 34_510_000, 34_520_000,
];

// 42 - 拳套 / Fist。包含 Unarmed，它在 CSV 中有名字且 motion category 非 0。
const WEAPON_POOL_42: &[u32] = &[
    110_000, 21_000_000, 21_010_000, 21_060_000, 21_070_000, 21_080_000, 21_100_000, 21_110_000,
    21_120_000, 21_130_000, 21_500_000, 21_510_000, 21_520_000, 21_530_000, 21_540_000,
];

// 43 - 鞭 / Whip
const WEAPON_POOL_43: &[u32] = &[
    20_000_000, 20_020_000, 20_030_000, 20_050_000, 20_060_000, 20_070_000, 20_500_000,
];

// 44 - 长弓 / Bow
const WEAPON_POOL_44: &[u32] = &[
    41_000_000, 41_010_000, 41_020_000, 41_030_000, 41_040_000, 41_060_000,
];

// 45 - 大弓 / Greatbow
const WEAPON_POOL_45: &[u32] = &[42_000_000, 42_010_000, 42_030_000, 42_040_000, 42_500_000];

// 46 - 弩 / Crossbow
const WEAPON_POOL_46: &[u32] = &[
    43_000_000, 43_020_000, 43_030_000, 43_050_000, 43_060_000, 43_080_000, 43_110_000, 43_500_000,
    43_510_000,
];

// 47 - 大盾 / Greatshield
const WEAPON_POOL_47: &[u32] = &[
    32_000_000, 32_020_000, 32_030_000, 32_040_000, 32_050_000, 32_080_000, 32_090_000, 32_120_000,
    32_130_000, 32_140_000, 32_150_000, 32_160_000, 32_170_000, 32_190_000, 32_200_000, 32_210_000,
    32_220_000, 32_230_000, 32_240_000, 32_250_000, 32_260_000, 32_270_000, 32_280_000, 32_290_000,
    32_300_000, 32_500_000, 32_520_000,
];

// 48 - 小盾 / Small Shield
const WEAPON_POOL_48: &[u32] = &[
    21_550_000, 24_510_000, 30_000_000, 30_010_000, 30_020_000, 30_030_000, 30_040_000, 30_070_000,
    30_080_000, 30_090_000, 30_100_000, 30_110_000, 30_120_000, 30_130_000, 30_140_000, 30_150_000,
    30_190_000, 30_200_000, 30_510_000, 31_170_000,
];

// 49 - 中盾 / Medium Shield
const WEAPON_POOL_49: &[u32] = &[
    30_060_000, 31_000_000, 31_010_000, 31_020_000, 31_030_000, 31_040_000, 31_050_000, 31_060_000,
    31_070_000, 31_080_000, 31_090_000, 31_100_000, 31_130_000, 31_140_000, 31_190_000, 31_230_000,
    31_240_000, 31_250_000, 31_260_000, 31_270_000, 31_280_000, 31_290_000, 31_300_000, 31_310_000,
    31_320_000, 31_330_000, 31_340_000, 31_500_000, 31_510_000, 31_520_000, 31_530_000,
];

// 50 - 镰刀 / Reaper
const WEAPON_POOL_50: &[u32] = &[19_000_000, 19_010_000, 19_020_000, 19_060_000, 19_500_000];

// 51 - 短弓 / Light Bow
const WEAPON_POOL_51: &[u32] = &[
    40_000_000, 40_010_000, 40_020_000, 40_030_000, 40_050_000, 40_500_000, 41_070_000, 41_510_000,
];

// 52 - 弩炮 / Ballista
const WEAPON_POOL_52: &[u32] = &[44_000_000, 44_010_000, 44_500_000];

// 53 - 投掷短剑 / Throwing Blade
const WEAPON_POOL_53: &[u32] = &[63_500_000];

// 55 - 格斗术 / Hand-to-Hand
const WEAPON_POOL_55: &[u32] = &[60_500_000, 60_510_000];

// 56 - 调香瓶 / Perfume Bottle
const WEAPON_POOL_56: &[u32] = &[61_500_000, 61_510_000, 61_520_000, 61_530_000, 61_540_000];

// 57 - 突刺盾 / Thrusting Shield
const WEAPON_POOL_57: &[u32] = &[62_500_000, 62_510_000];

// 58 - 反手剑 / Backhand Blade
const WEAPON_POOL_58: &[u32] = &[64_500_000, 64_510_000, 64_520_000];

// 60 - 轻大剑 / Light Greatsword
const WEAPON_POOL_60: &[u32] = &[67_500_000, 67_510_000, 67_520_000];

// 61 - 大太刀 / Great Katana
const WEAPON_POOL_61: &[u32] = &[66_500_000, 66_510_000, 66_520_000];

// 62 - 野兽爪 / Beast Claw
const WEAPON_POOL_62: &[u32] = &[68_500_000, 68_510_000];
