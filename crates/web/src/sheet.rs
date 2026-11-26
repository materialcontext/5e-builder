//! Printable character sheet (US Letter / A4, several pages).

use crate::editor::ordinal;
use crate::entries::Entries;
use crate::store::Store;
use compendium::{signed, Compendium, Entry};
use leptos::prelude::*;
use rules::sheet::{Proficiency, Sheet};
use rules::Character;
use std::sync::Arc;

#[component]
pub fn SheetPage(id: String) -> impl IntoView {
    let comp = expect_context::<Arc<Compendium>>();
    let store = expect_context::<Store>();
    let Some(ch) = store.with_untracked(|s| s.iter().find(|c| c.id == id).cloned()) else {
        return view! { <div class="page"><p>"Character not found. "</p><a href="#/">"Back"</a></div> }.into_any();
    };
    let full_spells = RwSignal::new(true);
    let full_features = RwSignal::new(true);
    let a4 = RwSignal::new(false);
    let b = rules::build(&ch, &comp);
    let s = rules::sheet(&ch, &b, &comp);
    let pending = rules::issues(&ch, &b, &comp).len();
    let edit_href = format!("#/edit/{id}/review");
    let ch2 = ch.clone();
    let s2 = s.clone();

    view! {
        <div class="sheet-toolbar no-print">
            <a class="button ghost" href="#/">"← Characters"</a>
            <a class="button ghost" href=edit_href>"Edit"</a>
            {(pending > 0).then(|| view! { <span class="badge warn">{format!("{pending} unfinished choices")}</span> })}
            <span class="spacer"></span>
            <label class="toggle small"><input type="checkbox" prop:checked=move || full_features.get() on:change=move |ev| full_features.set(event_target_checked(&ev)) />"Feature text"</label>
            <label class="toggle small"><input type="checkbox" prop:checked=move || full_spells.get() on:change=move |ev| full_spells.set(event_target_checked(&ev)) />"Spell descriptions"</label>
            <label class="toggle small"><input type="checkbox" prop:checked=move || a4.get() on:change=move |ev| a4.set(event_target_checked(&ev)) />"A4"</label>
            <button class="button primary" on:click=move |_| { if let Some(w) = web_sys::window() { let _ = w.print(); } }>"Print / Save PDF"</button>
        </div>
        {move || view! { <style>{if a4.get() { "@page { size: A4; margin: 0; } .sheet .page { width: 210mm; }" } else { "@page { size: letter; margin: 0; }" }}</style> }}
        <div class="sheet">
            {main_page(&ch, &s)}
            {move || features_page(&s2, full_features.get())}
            {spells_page(&s, full_spells)}
            {story_page(&ch2, &s)}
        </div>
    }
    .into_any()
}

fn boxed(label: &'static str, value: String) -> impl IntoView {
    view! { <div class="box"><div class="box-value">{value}</div><div class="box-label">{label}</div></div> }
}

fn main_page(ch: &Character, s: &Sheet) -> impl IntoView {
    let d = &ch.details;
    let abilities = s
        .abilities
        .iter()
        .map(|a| {
            let skills = s.skills.iter().filter(|k| k.skill.ability() == a.ability).map(|k| {
                let mark = match k.prof {
                    Proficiency::Expertise => "◆",
                    Proficiency::Proficient => "●",
                    Proficiency::Half => "◐",
                    Proficiency::None => "○",
                };
                view! { <li><span class="mark">{mark}</span><span class="val">{signed(k.bonus)}</span>{k.skill.name()}</li> }
            });
            view! {
                <div class="ability">
                    <div class="ability-box">
                        <div class="ability-name">{a.ability.name()}</div>
                        <div class="ability-mod">{signed(a.modifier)}</div>
                        <div class="ability-score">{a.score}</div>
                    </div>
                    <ul class="checks">
                        <li class="save"><span class="mark">{if a.save_prof { "●" } else { "○" }}</span><span class="val">{signed(a.save)}</span>"Saving throw"</li>
                        {skills.collect_view()}
                    </ul>
                </div>
            }
        })
        .collect_view();

    let attacks = s
        .attacks
        .iter()
        .map(|a| view! { <tr><td>{a.name.clone()}</td><td class="c">{signed(a.bonus)}</td><td>{a.damage.clone()}</td><td class="small">{a.notes.clone()}</td></tr> })
        .collect_view();

    let speed = {
        let mut parts = vec![format!("{} ft.", s.speed.walk)];
        for (n, v) in [("fly", s.speed.fly), ("swim", s.speed.swim), ("climb", s.speed.climb), ("burrow", s.speed.burrow)] {
            if v > 0 {
                parts.push(format!("{n} {v}"));
            }
        }
        parts.join(", ")
    };
    let senses = {
        let mut v = vec![format!("Passive Perception {}", s.passive_perception), format!("Passive Insight {}", s.passive_insight)];
        if s.darkvision > 0 {
            v.push(format!("Darkvision {} ft.", s.darkvision));
        }
        v.extend(s.senses.iter().cloned());
        v.join(" · ")
    };
    let list = |label: &'static str, items: &[String]| {
        (!items.is_empty()).then(|| view! { <p><b>{label}": "</b>{items.join(", ")}</p> })
    };
    let feature_names: Vec<String> = s.features.iter().map(|f| f.feature.name.clone()).collect();

    view! {
        <section class="page main">
            <header class="sheet-head">
                <div class="name-block"><div class="char-name">{s.name.clone()}</div><div class="box-label">"Character name"</div></div>
                <div class="meta">
                    <div><span>{s.class_line.clone()}</span><label>"Class & level"</label></div>
                    <div><span>{s.species.clone()}</span><label>"Species"</label></div>
                    <div><span>{s.background.clone()}</span><label>"Background"</label></div>
                    <div><span>{d.player.clone()}</span><label>"Player"</label></div>
                    <div><span>{d.alignment.clone()}</span><label>"Alignment"</label></div>
                    <div><span>{if d.xp > 0 { d.xp.to_string() } else { String::new() }}</span><label>"Experience"</label></div>
                </div>
            </header>
            <div class="cols">
                <div class="col-left">
                    <div class="pb">{boxed("Proficiency bonus", signed(s.proficiency_bonus))}</div>
                    {abilities}
                </div>
                <div class="col-right">
                    <div class="combat">
                        {boxed("Armor class", s.armor_class.to_string())}
                        {boxed("Initiative", signed(s.initiative))}
                        {boxed("Speed", speed)}
                        {boxed("Size", s.size.clone())}
                    </div>
                    <p class="small muted ac-note">{format!("AC: {}", s.ac_note)}</p>
                    <div class="hp">
                        <div class="hp-max"><span class="box-label">"Hit point maximum"</span><b>{s.hp_max}</b></div>
                        <div class="hp-current"><span class="box-label">"Current hit points"</span></div>
                        <div class="hp-temp"><span class="box-label">"Temporary"</span></div>
                    </div>
                    <div class="combat">
                        {boxed("Hit dice", s.hit_dice.clone())}
                        <div class="box death"><div class="box-value">"○○○ ○○○"</div><div class="box-label">"Death saves (✓ / ✗)"</div></div>
                    </div>
                    <div class="panel">
                        <h4>"Attacks"</h4>
                        <table class="attacks"><thead><tr><th>"Name"</th><th>"Bonus"</th><th>"Damage"</th><th>"Notes"</th></tr></thead><tbody>{attacks}</tbody></table>
                    </div>
                    <div class="panel">
                        <h4>"Senses"</h4>
                        <p>{senses}</p>
                        {list("Resistances", &s.resistances)}
                        {list("Immunities", &s.immunities)}
                        {list("Condition immunities", &s.condition_immunities)}
                        {list("Vulnerabilities", &s.vulnerabilities)}
                    </div>
                    <div class="panel">
                        <h4>"Proficiencies & languages"</h4>
                        {list("Armor", &s.armor_profs)}
                        {list("Weapons", &s.weapon_profs)}
                        {list("Tools", &s.tool_profs)}
                        {list("Languages", &s.languages)}
                    </div>
                    <div class="panel">
                        <h4>"Features & traits"</h4>
                        <p class="small">{feature_names.join(" · ")}</p>
                    </div>
                </div>
            </div>
        </section>
    }
}

fn features_page(s: &Sheet, full: bool) -> impl IntoView {
    let items = s
        .features
        .iter()
        .map(|f| {
            let body: Vec<Entry> = if full { f.feature.body.clone() } else { vec![] };
            view! {
                <div class="feature">
                    <h5>{f.feature.name.clone()}<span class="origin">{f.origin.clone()}</span></h5>
                    <Entries entries=body />
                </div>
            }
        })
        .collect_view();
    view! {
        <section class="page features">
            <h3 class="page-title">{format!("{} — Features & traits", s.name)}</h3>
            <div class="columns">{items}</div>
        </section>
    }
}

fn spells_page(s: &Sheet, full: RwSignal<bool>) -> impl IntoView {
    if s.spells.is_empty() && s.casting.is_empty() {
        return ().into_any();
    }
    let casting = s
        .casting
        .iter()
        .map(|c| {
            view! {
                <div class="casting-row">
                    <b>{c.label.clone()}</b>
                    {boxed("Ability", c.ability.abbr().to_string())}
                    {boxed("Save DC", c.save_dc.to_string())}
                    {boxed("Attack", signed(c.attack))}
                </div>
            }
        })
        .collect_view();
    let slots = s
        .slots
        .iter()
        .enumerate()
        .filter(|(_, n)| **n > 0)
        .map(|(i, n)| view! { <div class="slot"><b>{ordinal(i as u8 + 1)}</b><span>{"☐".repeat(*n as usize)}</span></div> })
        .collect_view();
    let pact = s.pact_slots.map(|(n, l)| view! { <div class="slot"><b>{format!("Pact ({})", ordinal(l))}</b><span>{"☐".repeat(n as usize)}</span></div> });
    let mut levels: Vec<u8> = s.spells.iter().map(|x| x.level).collect();
    levels.dedup();
    let spells = s.spells.clone();
    let tables = levels
        .into_iter()
        .map(|lvl| {
            let rows = spells
                .iter()
                .filter(|x| x.level == lvl)
                .map(|x| {
                    let tags = [x.concentration.then_some("C"), x.ritual.then_some("R")].into_iter().flatten().collect::<Vec<_>>().join(" ");
                    view! {
                        <tr>
                            <td class="c">{x.status.clone()}</td>
                            <td><b>{x.name.clone()}</b></td>
                            <td>{x.time.clone()}</td>
                            <td>{x.range.clone()}</td>
                            <td>{x.components.clone()}</td>
                            <td>{x.duration.clone()}</td>
                            <td class="c">{tags}</td>
                            <td class="small">{x.source_label.clone()}</td>
                        </tr>
                    }
                })
                .collect_view();
            view! {
                <h4>{if lvl == 0 { "Cantrips".to_string() } else { format!("{} level", ordinal(lvl)) }}</h4>
                <table class="spells">
                    <thead><tr><th></th><th>"Spell"</th><th>"Time"</th><th>"Range"</th><th>"Components"</th><th>"Duration"</th><th>"C/R"</th><th>"From"</th></tr></thead>
                    <tbody>{rows}</tbody>
                </table>
            }
        })
        .collect_view();
    let descriptions = move || {
        full.get().then(|| {
            let items = spells
                .iter()
                .map(|x| {
                    let lvl = if x.level == 0 { format!("{} cantrip", x.school) } else { format!("Level {} {}", x.level, x.school) };
                    view! {
                        <div class="spell-card">
                            <h5>{x.name.clone()}<span class="origin">{lvl}</span></h5>
                            <p class="small">{format!("{} · {} · {} · {}", x.time, x.range, x.components, x.duration)}</p>
                            <Entries entries=x.body.clone() />
                        </div>
                    }
                })
                .collect_view();
            view! { <section class="page spell-text"><h3 class="page-title">"Spell descriptions"</h3><div class="columns">{items}</div></section> }
        })
    };
    view! {
        <section class="page spells-page">
            <h3 class="page-title">{format!("{} — Spellcasting", s.name)}</h3>
            <div class="casting-block">{casting}</div>
            <div class="slots">{slots}{pact}</div>
            {tables}
            <p class="small muted">"C = concentration, R = ritual. Always = always prepared."</p>
        </section>
        {descriptions}
    }
    .into_any()
}

fn story_page(ch: &Character, s: &Sheet) -> impl IntoView {
    let d = &ch.details;
    let inv = s
        .inventory
        .iter()
        .map(|i| {
            view! {
                <tr>
                    <td>{if i.equipped { "●" } else { "" }}{if i.attuned { "Ⓐ" } else { "" }}</td>
                    <td>{i.name.clone()}{(!i.notes.is_empty()).then(|| view! { <div class="small muted">{i.notes.clone()}</div> })}</td>
                    <td class="c">{i.qty}</td>
                    <td class="c">{if i.weight > 0.0 { format!("{}", i.weight) } else { String::new() }}</td>
                </tr>
            }
        })
        .collect_view();
    let c = &s.coins;
    let text = |label: &'static str, v: &str| view! { <div class="text-block"><h4>{label}</h4><p class="prewrap">{v.to_string()}</p></div> };
    let look = [
        ("Age", &d.age),
        ("Height", &d.height),
        ("Weight", &d.weight),
        ("Eyes", &d.eyes),
        ("Hair", &d.hair),
        ("Skin", &d.skin),
        ("Pronouns", &d.pronouns),
        ("Faith", &d.faith),
    ]
    .iter()
    .map(|(l, v)| view! { <div><span>{v.to_string()}</span><label>{*l}</label></div> })
    .collect_view();
    view! {
        <section class="page story">
            <h3 class="page-title">{format!("{} — Equipment & story", s.name)}</h3>
            <div class="cols">
                <div class="col-half">
                    <h4>"Equipment"</h4>
                    <table class="inventory"><thead><tr><th>"E"</th><th>"Item"</th><th>"Qty"</th><th>"Wt."</th></tr></thead><tbody>{inv}</tbody></table>
                    <p class="small">{format!("Carried {:.1} lb. · Capacity {} lb.", s.carried_weight, s.carry_capacity)}</p>
                    <div class="coins-print">
                        {boxed("CP", c.cp.to_string())}{boxed("SP", c.sp.to_string())}{boxed("EP", c.ep.to_string())}{boxed("GP", c.gp.to_string())}{boxed("PP", c.pp.to_string())}
                    </div>
                </div>
                <div class="col-half">
                    <div class="meta look">{look}</div>
                    {text("Personality traits", &d.personality)}
                    {text("Ideals", &d.ideals)}
                    {text("Bonds", &d.bonds)}
                    {text("Flaws", &d.flaws)}
                    {text("Appearance", &d.appearance)}
                </div>
            </div>
            {text("Backstory", &d.backstory)}
            {text("Allies & organizations", &d.allies)}
            {text("Notes", &d.notes)}
        </section>
    }
}
