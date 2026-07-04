// =============================================================================
// SOVEREIGN ACCOUNTS — Детерминированные PalletId адреса
// =============================================================================
//
// Каждый субъект суверенитета в Altan L1 получает своё уникальное
// keyless пространство аккаунтов, выведенных через PalletId.
//
// ## Схема деривации
//
// ```
// PalletId(prefix).into_account_truncating()          → главный аккаунт
// PalletId(prefix).into_sub_account_truncating(id)    → суб-аккаунт #id
// ```
//
// ## Три уровня суверенитета
//
// | Уровень | Счёт | PalletId prefix | Слоты |
// |---------|------|-----------------|-------|
// | 79 Коренных народов (Khuraltai) | 1–79  | `alt/indN`* | treasury + council |
// | 88 Натурализованных групп       | 1001–1088 | `alt/natN`* | treasury |
// | 193 Иностр. государства (ООН)   | 1–193 (ISO 3166) | `alt/dipN`* | 13 дипл. слота |
//
// *N = 2-byte big-endian nation/country index.
//
// ## Дипломатические слоты (13 per country)
// - Slot 0:  MFA Account       — МИД / Министерство Иностранных Дел
// - Slot 1:  Embassy Account   — Посольство / Консульство
// - Slot 2:  Passport Office   — Загранпаспортный стол
// - Slot 3–12: Bank Wallet #1–#10 — Дипломатические банковские счета
//
// ## Конституционные ограничения
// - Государственные аккаунты НЕ могут использовать shielded vaults (TransparentStateGuard).
// - `Sanctioned` государства НЕ могут открывать банковские операции.
// - Натурализованные группы NOT участвуют в Хурале (уровень 5+).
// - Коренные народы имеют ПОЛНЫЙ политический суверенитет.
//
// =============================================================================

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)]

use frame_support::PalletId;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::AccountId32;

// Re-export the AccountId type alias used throughout the runtime.
pub type AccountId = AccountId32;

// =============================================================================
// SECTION 1: 79 Коренных Народов (Indigenous Nations / Khuraltai)
// =============================================================================
//
// IDs 1–79. Полный политический суверенитет.
// Каждый народ получает:
//   - Treasury: хранилище казны народа
//   - Council:  мультисиг Совета Старейшин
//
// PalletId схема: `alt/ind` + 2-byte big-endian nation_id
// Пример: nation_id=1 → PalletId(*b"alt/ind\x00\x01")

/// Derive the treasury account for an indigenous nation.
///
/// `nation_id`: 1–79
/// Address = PalletId(*b"alt/ind!").into_sub_account_truncating(nation_id)
pub fn indigenous_treasury(nation_id: u16) -> AccountId {
    PalletId(*b"alt/ind!").into_sub_account_truncating(nation_id as u32)
}

/// Derive the council multisig account for an indigenous nation.
///
/// Sub-account key: nation_id | 0x8000 (high bit marks council).
/// `nation_id`: 1–79
pub fn indigenous_council(nation_id: u16) -> AccountId {
    PalletId(*b"alt/ind!").into_sub_account_truncating(0x8000u32 | nation_id as u32)
}

/// All genesis accounts for 79 indigenous nations.
/// Returns (treasury, council) pairs.
pub fn all_indigenous_accounts() -> impl Iterator<Item = (u16, AccountId, AccountId)> {
    (1u16..=79u16).map(|id| (id, indigenous_treasury(id), indigenous_council(id)))
}

// =============================================================================
// SECTION 2: 88 Натурализованных Групп (Naturalized Groups)
// =============================================================================
//
// IDs 1001–1088. Экономические права, НЕ участвуют в Хурале.
// Каждая группа получает один treasury аккаунт.
//
// PalletId схема: `alt/nat` + 2-byte big-endian group_id
// Пример: group_id=1001 → prefix[6]=0x03, prefix[7]=0xe9

/// Derive the treasury account for a naturalized group.
///
/// `group_id`: 1001–1088
/// Address = PalletId(*b"alt/nat!").into_sub_account_truncating(group_id)
pub fn naturalized_treasury(group_id: u16) -> AccountId {
    PalletId(*b"alt/nat!").into_sub_account_truncating(group_id as u32)
}

/// All genesis accounts for 88 naturalized groups.
pub fn all_naturalized_accounts() -> impl Iterator<Item = (u16, AccountId)> {
    (1001u16..=1088u16).map(|id| (id, naturalized_treasury(id)))
}

// =============================================================================
// SECTION 3: 193 Иностранных Государства (UN Member States)
// =============================================================================
//
// ISO 3166-1 numeric codes 1–999 (193 members + observers used).
// Each state gets 13 diplomatic account slots.
//
// PalletId схема: `alt/dip` + 2-byte big-endian country_code
// Sub-account index = slot (0–12):
//   0: MFA (МИД)
//   1: Embassy (Посольство)
//   2: Passport Office
//   3–12: Bank Wallet #1–#10

/// Number of diplomatic slots per foreign state.
pub const DIPLOMATIC_SLOTS: u8 = 13;

/// Derive a single diplomatic account for a foreign state.
///
/// `country_code`: ISO 3166-1 numeric (e.g. 643 = Russia, 840 = USA)
/// `slot`: 0–12 (see module docs)
///
/// Key = (country_code as u32) << 8 | slot as u32
/// This encodes 193 countries × 13 slots with no collision.
pub fn diplomatic_account(country_code: u16, slot: u8) -> AccountId {
    debug_assert!(slot < DIPLOMATIC_SLOTS, "Slot must be 0–12");
    let key = ((country_code as u32) << 8) | (slot as u32);
    PalletId(*b"alt/dip!").into_sub_account_truncating(key)
}

/// Convenience: MFA account for a country.
pub fn diplomatic_mfa(country_code: u16) -> AccountId {
    diplomatic_account(country_code, 0)
}

/// Convenience: Embassy account for a country.
pub fn diplomatic_embassy(country_code: u16) -> AccountId {
    diplomatic_account(country_code, 1)
}

/// Convenience: Passport Office account for a country.
pub fn diplomatic_passport(country_code: u16) -> AccountId {
    diplomatic_account(country_code, 2)
}

/// Convenience: Banking wallet N (1–10) for a country.
pub fn diplomatic_bank(country_code: u16, bank_n: u8) -> AccountId {
    debug_assert!(bank_n >= 1 && bank_n <= 10, "bank_n must be 1–10");
    diplomatic_account(country_code, 2 + bank_n) // slot 3=bank1, 4=bank2, ..., 12=bank10
}

/// All 13 diplomatic accounts for a single country.
pub fn all_diplomatic_slots(country_code: u16) -> [(u8, AccountId); 13] {
    core::array::from_fn(|i| (i as u8, diplomatic_account(country_code, i as u8)))
}

// =============================================================================
// UN MEMBER STATES — ISO 3166-1 Numeric Codes
// =============================================================================
//
// 193 UN member states + 2 observer states (Vatican + Palestine).
// Each entry: (iso_numeric, name_en, name_ru)
//
// Source: UN Member States list (as of 2024).
// =============================================================================

/// (iso_numeric_code, english_name, russian_name)
pub const UN_MEMBER_STATES: &[(u16, &str, &str)] = &[
    // Africa (54 states)
    (012, "Algeria", "Алжир"),
    (024, "Angola", "Ангола"),
    (204, "Benin", "Бенин"),
    (072, "Botswana", "Ботсвана"),
    (854, "Burkina Faso", "Буркина-Фасо"),
    (108, "Burundi", "Бурунди"),
    (132, "Cabo Verde", "Кабо-Верде"),
    (120, "Cameroon", "Камерун"),
    (140, "Central African Republic", "ЦАР"),
    (148, "Chad", "Чад"),
    (174, "Comoros", "Коморы"),
    (178, "Congo", "Республика Конго"),
    (180, "DR Congo", "ДР Конго"),
    (384, "Ivory Coast", "Кот-д'Ивуар"),
    (262, "Djibouti", "Джибути"),
    (818, "Egypt", "Египет"),
    (226, "Equatorial Guinea", "Экваториальная Гвинея"),
    (232, "Eritrea", "Эритрея"),
    (231, "Ethiopia", "Эфиопия"),
    (266, "Gabon", "Габон"),
    (270, "Gambia", "Гамбия"),
    (288, "Ghana", "Гана"),
    (324, "Guinea", "Гвинея"),
    (624, "Guinea-Bissau", "Гвинея-Бисау"),
    (404, "Kenya", "Кения"),
    (426, "Lesotho", "Лесото"),
    (430, "Liberia", "Либерия"),
    (434, "Libya", "Ливия"),
    (450, "Madagascar", "Мадагаскар"),
    (454, "Malawi", "Малави"),
    (466, "Mali", "Мали"),
    (478, "Mauritania", "Мавритания"),
    (480, "Mauritius", "Маврикий"),
    (504, "Morocco", "Марокко"),
    (508, "Mozambique", "Мозамбик"),
    (516, "Namibia", "Намибия"),
    (562, "Niger", "Нигер"),
    (566, "Nigeria", "Нигерия"),
    (646, "Rwanda", "Руанда"),
    (678, "Sao Tome and Principe", "Сан-Томе и Принсипи"),
    (686, "Senegal", "Сенегал"),
    (694, "Sierra Leone", "Сьерра-Леоне"),
    (706, "Somalia", "Сомали"),
    (710, "South Africa", "ЮАР"),
    (728, "South Sudan", "Южный Судан"),
    (729, "Sudan", "Судан"),
    (748, "Eswatini", "Эсватини"),
    (834, "Tanzania", "Танзания"),
    (768, "Togo", "Того"),
    (788, "Tunisia", "Тунис"),
    (800, "Uganda", "Уганда"),
    (894, "Zambia", "Замбия"),
    (716, "Zimbabwe", "Зимбабве"),
    // Americas (35 states)
    (028, "Antigua and Barbuda", "Антигуа и Барбуда"),
    (032, "Argentina", "Аргентина"),
    (044, "Bahamas", "Багамы"),
    (052, "Barbados", "Барбадос"),
    (084, "Belize", "Белиз"),
    (068, "Bolivia", "Боливия"),
    (076, "Brazil", "Бразилия"),
    (124, "Canada", "Канада"),
    (152, "Chile", "Чили"),
    (170, "Colombia", "Колумбия"),
    (188, "Costa Rica", "Коста-Рика"),
    (192, "Cuba", "Куба"),
    (214, "Dominican Republic", "Доминиканская Республика"),
    (218, "Ecuador", "Эквадор"),
    (222, "El Salvador", "Сальвадор"),
    (308, "Grenada", "Гренада"),
    (320, "Guatemala", "Гватемала"),
    (328, "Guyana", "Гайана"),
    (332, "Haiti", "Гаити"),
    (340, "Honduras", "Гондурас"),
    (388, "Jamaica", "Ямайка"),
    (484, "Mexico", "Мексика"),
    (558, "Nicaragua", "Никарагуа"),
    (591, "Panama", "Панама"),
    (600, "Paraguay", "Парагвай"),
    (604, "Peru", "Перу"),
    (659, "Saint Kitts and Nevis", "Сент-Китс и Невис"),
    (662, "Saint Lucia", "Сент-Люсия"),
    (
        670,
        "Saint Vincent and the Grenadines",
        "Сент-Винсент и Гренадины",
    ),
    (740, "Suriname", "Суринам"),
    (780, "Trinidad and Tobago", "Тринидад и Тобаго"),
    (858, "Uruguay", "Уругвай"),
    (840, "United States", "США"),
    (862, "Venezuela", "Венесуэла"),
    (212, "Dominica", "Доминика"),
    // Asia & Pacific (53 states)
    (004, "Afghanistan", "Афганистан"),
    (050, "Bangladesh", "Бангладеш"),
    (064, "Bhutan", "Бутан"),
    (096, "Brunei", "Бруней"),
    (116, "Cambodia", "Камбоджа"),
    (156, "China", "Китай"),
    (242, "Fiji", "Фиджи"),
    (356, "India", "Индия"),
    (360, "Indonesia", "Индонезия"),
    (364, "Iran", "Иран"),
    (368, "Iraq", "Ирак"),
    (376, "Israel", "Израиль"),
    (392, "Japan", "Япония"),
    (400, "Jordan", "Иордания"),
    (398, "Kazakhstan", "Казахстан"),
    (296, "Kiribati", "Кирибати"),
    (408, "North Korea", "КНДР"),
    (410, "South Korea", "Республика Корея"),
    (414, "Kuwait", "Кувейт"),
    (417, "Kyrgyzstan", "Кыргызстан"),
    (418, "Laos", "Лаос"),
    (422, "Lebanon", "Ливан"),
    (458, "Malaysia", "Малайзия"),
    (462, "Maldives", "Мальдивы"),
    (496, "Mongolia", "Монголия"),
    (104, "Myanmar", "Мьянма"),
    (524, "Nepal", "Непал"),
    (554, "New Zealand", "Новая Зеландия"),
    (512, "Oman", "Оман"),
    (586, "Pakistan", "Пакистан"),
    (585, "Palau", "Палау"),
    (598, "Papua New Guinea", "Папуа — Новая Гвинея"),
    (608, "Philippines", "Филиппины"),
    (634, "Qatar", "Катар"),
    (882, "Samoa", "Самоа"),
    (682, "Saudi Arabia", "Саудовская Аравия"),
    (690, "Seychelles", "Сейшелы"),
    (702, "Singapore", "Сингапур"),
    (090, "Solomon Islands", "Соломоновы Острова"),
    (144, "Sri Lanka", "Шри-Ланка"),
    (275, "Palestine", "Палестина"),
    (760, "Syria", "Сирия"),
    (762, "Tajikistan", "Таджикистан"),
    (764, "Thailand", "Таиланд"),
    (626, "Timor-Leste", "Восточный Тимор"),
    (776, "Tonga", "Тонга"),
    (798, "Tuvalu", "Тувалу"),
    (784, "UAE", "ОАЭ"),
    (860, "Uzbekistan", "Узбекистан"),
    (548, "Vanuatu", "Вануату"),
    (704, "Vietnam", "Вьетнам"),
    (887, "Yemen", "Йемен"),
    (795, "Turkmenistan", "Туркменистан"),
    // Europe (44 states)
    (008, "Albania", "Албания"),
    (020, "Andorra", "Андорра"),
    (040, "Austria", "Австрия"),
    (031, "Azerbaijan", "Азербайджан"),
    (112, "Belarus", "Беларусь"),
    (056, "Belgium", "Бельгия"),
    (070, "Bosnia and Herzegovina", "Босния и Герцеговина"),
    (100, "Bulgaria", "Болгария"),
    (191, "Croatia", "Хорватия"),
    (196, "Cyprus", "Кипр"),
    (203, "Czech Republic", "Чехия"),
    (208, "Denmark", "Дания"),
    (233, "Estonia", "Эстония"),
    (246, "Finland", "Финляндия"),
    (250, "France", "Франция"),
    (268, "Georgia", "Грузия"),
    (276, "Germany", "Германия"),
    (300, "Greece", "Греция"),
    (348, "Hungary", "Венгрия"),
    (352, "Iceland", "Исландия"),
    (372, "Ireland", "Ирландия"),
    (380, "Italy", "Италия"),
    (428, "Latvia", "Латвия"),
    (438, "Liechtenstein", "Лихтенштейн"),
    (440, "Lithuania", "Литва"),
    (442, "Luxembourg", "Люксембург"),
    (807, "North Macedonia", "Северная Македония"),
    (470, "Malta", "Мальта"),
    (498, "Moldova", "Молдова"),
    (492, "Monaco", "Монако"),
    (499, "Montenegro", "Черногория"),
    (528, "Netherlands", "Нидерланды"),
    (578, "Norway", "Норвегия"),
    (616, "Poland", "Польша"),
    (620, "Portugal", "Португалия"),
    (642, "Romania", "Румыния"),
    (643, "Russia", "Россия"),
    (674, "San Marino", "Сан-Марино"),
    (688, "Serbia", "Сербия"),
    (703, "Slovakia", "Словакия"),
    (705, "Slovenia", "Словения"),
    (724, "Spain", "Испания"),
    (752, "Sweden", "Швеция"),
    (756, "Switzerland", "Швейцария"),
    (792, "Turkey", "Турция"),
    (804, "Ukraine", "Украина"),
    (826, "United Kingdom", "Великобритания"),
    (336, "Vatican", "Ватикан"),
    // Oceania (additional)
    (036, "Australia", "Австралия"),
    (583, "Micronesia", "Микронезия"),
    (584, "Marshall Islands", "Маршалловы Острова"),
    (520, "Nauru", "Науру"),
];

/// Total count of UN member states in this registry.
pub const UN_STATES_COUNT: usize = UN_MEMBER_STATES.len();

// =============================================================================
// 79 КОРЕННЫХ НАРОДОВ СИБИРИ
// =============================================================================
//
// (nation_id 1–79, ru_name, en_name)
// Source: Russian Federation registry of indigenous peoples of Siberia + INOMAD Constitution.
//
// =============================================================================

/// (nation_id, russian_name, english_name)
pub const INDIGENOUS_NATIONS_79: &[(u16, &str, &str)] = &[
    // Тюркские народы Сибири
    (1, "Буряты", "Buryats"),
    (2, "Якуты (Саха)", "Yakuts (Sakha)"),
    (3, "Тувинцы", "Tuvans"),
    (4, "Хакасы", "Khakassians"),
    (5, "Алтайцы", "Altaians"),
    (6, "Шорцы", "Shors"),
    (7, "Теленгиты", "Telengits"),
    (8, "Телеуты", "Teleuts"),
    (9, "Тубалары", "Tubalars"),
    (10, "Кумандинцы", "Kumandinets"),
    (11, "Челканцы", "Chelkans"),
    (12, "Долганы", "Dolgans"),
    (13, "Тофалары", "Tofalars"),
    (14, "Сойоты", "Soyots"),
    (15, "Чулымцы", "Chulymtsy"),
    // Монгольские народы
    (16, "Буряад-Монголы", "Buryad-Mongols"),
    (17, "Баргуты", "Barguts"),
    (18, "Калмыки", "Kalmyks"),
    (19, "Дауры", "Daurs"),
    (20, "Хамниганы", "Khamnigans"),
    // Тунгусо-маньчжурские народы
    (21, "Эвенки", "Evenks"),
    (22, "Эвены", "Evens"),
    (23, "Нанайцы", "Nanais"),
    (24, "Ульчи", "Ulchis"),
    (25, "Орочи", "Orochis"),
    (26, "Удэгейцы", "Udegeis"),
    (27, "Орки (Уйльта)", "Oroks (Uilta)"),
    (28, "Негидальцы", "Negidalians"),
    (29, "Сибо", "Xibe"),
    // Самодийские народы
    (30, "Ненцы", "Nenets"),
    (31, "Энцы", "Entsy"),
    (32, "Нганасаны", "Nganasans"),
    (33, "Селькупы", "Selkups"),
    // Финно-угорские народы Сибири
    (34, "Ханты", "Khanty"),
    (35, "Манси", "Mansi"),
    // Юкагирские народы
    (36, "Юкагиры", "Yukaghirs"),
    // Палеосибирские народы
    (37, "Чукчи", "Chukchi"),
    (38, "Коряки", "Koryaks"),
    (39, "Ительмены", "Itelmens"),
    (40, "Нивхи", "Nivkhs"),
    (41, "Кеты", "Kets"),
    (42, "Юпики (Азиатские)", "Yupiks (Asian)"),
    // Эскимосско-алеутские
    (43, "Алеуты", "Aleuts"),
    // Народы Алтая и Саян
    (44, "Теле́сы", "Telesy"),
    (45, "Кижи", "Kizhi"),
    (46, "Урянхайцы", "Uriankhai"),
    (47, "Онгуты", "Onguts"),
    (48, "Кыргызы-Саянские", "Sayan Kyrgyz"),
    // Народы Байкала и Забайкалья
    (49, "Хонгирад", "Khongirad"),
    (50, "Борджигин", "Borjigin"),
    (51, "Меркиты", "Merkit"),
    (52, "Найманы", "Naimans"),
    (53, "Кереиты", "Keraits"),
    (54, "Ойраты", "Oirats"),
    (55, "Дэрбэты", "Derbets"),
    (56, "Хошуты", "Khoshuts"),
    (57, "Торгуты", "Torghuts"),
    (58, "Джунгары", "Dzungars"),
    // Народы Северо-Востока
    (59, "Ламуты", "Lamuts"),
    (60, "Чуванцы", "Chuvanets"),
    (61, "Кереки", "Kereks"),
    // Народы Сахалина и Курил
    (62, "Айны", "Ainu"),
    // Народы Красноярского края
    (63, "Кольчегинцы", "Kolcheginets"),
    (64, "Карагасы", "Karagasy"),
    // Народы Западной Сибири
    (65, "Ваховские ханты", "Vakh Khanty"),
    (66, "Казымские ханты", "Kazym Khanty"),
    // Народы Восточной Сибири
    (67, "Туматы", "Tumats"),
    (68, "Урсуты", "Ursuts"),
    (69, "Кыштымы", "Kyshtym"),
    (70, "Аринцы", "Arintsy"),
    (71, "Котовцы", "Kotovtsy"),
    (72, "Ассаны", "Assans"),
    (73, "Буланцы", "Bulantsy"),
    // Народы Приамурья и Приморья
    (74, "Тазы", "Tazy"),
    (75, "Бикинские нанайцы", "Bikin Nanais"),
    // Народы Аляски — Сибирская диаспора
    (76, "Инупиаты", "Inupiats"),
    (77, "Атабаски", "Athabascans"),
    // Малочисленные народы (reserved constitutional slots)
    (78, "Абазины", "Abazins"),
    (79, "Нымыланы", "Nymylans"),
];

/// (group_id 1001–1088, russian_name, english_name)  
pub const NATURALIZED_GROUPS_88: &[(u16, &str, &str)] = &[
    (1001, "Русские", "Russians"),
    (1002, "Украинцы", "Ukrainians"),
    (1003, "Белорусы", "Belarusians"),
    (1004, "Казахи", "Kazakhs"),
    (1005, "Узбеки", "Uzbeks"),
    (1006, "Армяне", "Armenians"),
    (1007, "Азербайджанцы", "Azerbaijanis"),
    (1008, "Грузины", "Georgians"),
    (1009, "Таджики", "Tajiks"),
    (1010, "Киргизы", "Kyrgyz"),
    (1011, "Туркмены", "Turkmens"),
    (1012, "Молдаване", "Moldovans"),
    (1013, "Латыши", "Latvians"),
    (1014, "Литовцы", "Lithuanians"),
    (1015, "Эстонцы", "Estonians"),
    (1016, "Татары", "Tatars"),
    (1017, "Чуваши", "Chuvash"),
    (1018, "Башкиры", "Bashkirs"),
    (1019, "Мордва", "Mordvins"),
    (1020, "Удмурты", "Udmurts"),
    (1021, "Марийцы", "Mari"),
    (1022, "Коми", "Komi"),
    (1023, "Коми-Пермяки", "Komi-Permyaks"),
    (1024, "Карелы", "Karelians"),
    (1025, "Вепсы", "Vespians"),
    (1026, "Финны", "Finns"),
    (1027, "Ижорцы", "Izhorians"),
    (1028, "Водь", "Votes"),
    (1029, "Саамы", "Sami"),
    (1030, "Ненцы (натурализованные)", "Nenets (naturalized)"),
    (1031, "Чеченцы", "Chechens"),
    (1032, "Аварцы", "Avars"),
    (1033, "Лезгины", "Lezgins"),
    (1034, "Даргинцы", "Dargins"),
    (1035, "Кумыки", "Kumyks"),
    (1036, "Ингуши", "Ingush"),
    (1037, "Кабардинцы", "Kabardians"),
    (1038, "Осетины", "Ossetians"),
    (1039, "Адыгейцы", "Adygeans"),
    (1040, "Карачаевцы", "Karachais"),
    (1041, "Балкарцы", "Balkars"),
    (1042, "Черкесы", "Circassians"),
    (1043, "Абхазы", "Abkhazians"),
    (1044, "Лакцы", "Laks"),
    (1045, "Табасараны", "Tabasarans"),
    (1046, "Рутульцы", "Rutuls"),
    (1047, "Агулы", "Aguls"),
    (1048, "Цахуры", "Tsakhurs"),
    (1049, "Ногайцы", "Nogais"),
    (1050, "Шапсуги", "Shapsug"),
    (1051, "Китайцы", "Chinese"),
    (1052, "Корейцы", "Koreans"),
    (1053, "Японцы", "Japanese"),
    (1054, "Вьетнамцы", "Vietnamese"),
    (1055, "Монголы", "Mongols"),
    (1056, "Тибетцы", "Tibetans"),
    (1057, "Уйгуры", "Uyghurs"),
    (1058, "Евреи", "Jews"),
    (1059, "Цыгане", "Roma"),
    (1060, "Немцы", "Germans"),
    (1061, "Поляки", "Poles"),
    (1062, "Чехи", "Czechs"),
    (1063, "Словаки", "Slovaks"),
    (1064, "Венгры", "Hungarians"),
    (1065, "Румыны", "Romanians"),
    (1066, "Болгары", "Bulgarians"),
    (1067, "Сербы", "Serbs"),
    (1068, "Хорваты", "Croats"),
    (1069, "Греки", "Greeks"),
    (1070, "Итальянцы", "Italians"),
    (1071, "Испанцы", "Spaniards"),
    (1072, "Французы", "French"),
    (1073, "Англичане", "English"),
    (1074, "Арабы", "Arabs"),
    (1075, "Персы", "Persians"),
    (1076, "Курды", "Kurds"),
    (1077, "Турки", "Turks"),
    (1078, "Индийцы (хинди)", "Indians (Hindi)"),
    (1079, "Бенгальцы", "Bengalis"),
    (1080, "Пакистанцы", "Pakistanis"),
    (1081, "Американцы", "Americans"),
    (1082, "Канадцы", "Canadians"),
    (1083, "Австралийцы", "Australians"),
    (1084, "Бразильцы", "Brazilians"),
    (1085, "Индонезийцы", "Indonesians"),
    (1086, "Малайцы", "Malays"),
    (1087, "Филиппинцы", "Filipinos"),
    (1088, "Афроамериканцы", "African Americans"),
];

// =============================================================================
// GENESIS HELPERS — для genesis_config_presets.rs
// =============================================================================

/// Generate all genesis balance entries for sovereign accounts.
///
/// Each account receives `existential_deposit` (1 ALTAN = 10^12 planck)
/// as the minimum to prevent account reaping.
///
/// Returns iterator of (account, balance).
pub fn all_sovereign_genesis_balances(
    existential_deposit: u128,
) -> impl Iterator<Item = (AccountId, u128)> {
    let indigenous = (1u16..=79u16).flat_map(move |id| {
        [indigenous_treasury(id), indigenous_council(id)]
            .into_iter()
            .map(move |acc| (acc, existential_deposit))
    });

    let naturalized =
        (1001u16..=1088u16).map(move |id| (naturalized_treasury(id), existential_deposit));

    let diplomatic = UN_MEMBER_STATES.iter().flat_map(move |&(code, _, _)| {
        (0u8..DIPLOMATIC_SLOTS)
            .map(move |slot| (diplomatic_account(code, slot), existential_deposit))
    });

    indigenous.chain(naturalized).chain(diplomatic)
}

/// Summary counts for verification.
pub fn sovereign_account_summary() -> (usize, usize, usize, usize) {
    let indigenous_count = 79 * 2; // treasury + council
    let naturalized_count = 88;
    let diplomatic_count = UN_MEMBER_STATES.len() * DIPLOMATIC_SLOTS as usize;
    let total = indigenous_count + naturalized_count + diplomatic_count;
    (indigenous_count, naturalized_count, diplomatic_count, total)
}
