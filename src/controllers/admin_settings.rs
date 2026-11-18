use admin_module::{CatalogFont, FontCatalog};
use maud::{Markup, PreEscaped, html};
use serde::Deserialize;

use crate::controllers::AppState;
use crate::controllers::admin_controller::{AdminAuth, logout_form};
use crate::i18n::{Locale, Texts, fill};
use crate::settings::{FONTSOURCE_CDN_URL, FontRequest, FontRequests, SettingsRequest};
use crate::site_config::{
    Colors, Features, Font, FontFallback, FontStyle, Fonts, Identity, Link, MAX_RADIUS, SiteConfig,
};

pub const KEEP_FONT: &str = "keep";
const WEIGHTS: [u16; 9] = [100, 200, 300, 400, 500, 600, 700, 800, 900];
const MAX_FONT_INPUT: usize = 100;
const RESET_STYLES: &str = "styles";
const RESET_DETAILS: &str = "details";
const RESET_BUTTON_CLASS: &str =
    "rounded-box border border-shade-color px-3 py-1 cursor-pointer hover:border-primary-color";
const INPUT_CLASS: &str = "rounded-box border border-shade-color bg-background-color px-2 py-1 outline-none focus:border-primary-color";

#[derive(Deserialize)]
pub struct SettingsForm {
    site_name: String,
    site_source_url: String,
    description_en: String,
    description_es: String,
    site_links: String,
    color_background: String,
    color_bright: String,
    color_primary: String,
    color_secondary: String,
    color_accent: String,
    color_shade: String,
    radius: Option<u8>,
    body_font: String,
    body_weight: Option<u16>,
    body_fallback: FontFallback,
    heading_font: String,
    heading_weight: Option<u16>,
    heading_fallback: FontFallback,
    feature_news: Option<String>,
    feature_chess: Option<String>,
    feature_music: Option<String>,
    feature_games: Option<String>,
    feature_schedule: Option<String>,
}

impl SettingsForm {
    pub fn into_values(self, site: &SiteConfig) -> FormValues {
        let mut description = site.identity.description.clone();
        for (lang, text) in [("en", self.description_en), ("es", self.description_es)] {
            let text = text.trim();
            if text.is_empty() {
                description.remove(lang);
            } else {
                description.insert(lang.to_string(), text.to_string());
            }
        }
        let source_url = self.site_source_url.trim();

        FormValues {
            identity: Identity {
                name: self.site_name.trim().to_string(),
                source_url: (!source_url.is_empty()).then(|| source_url.to_string()),
                description,
                links: parse_links(&self.site_links),
            },
            colors: Colors {
                background: self.color_background,
                bright: self.color_bright,
                primary: self.color_primary,
                secondary: self.color_secondary,
                accent: self.color_accent,
                shade: self.color_shade,
            },
            radius: self.radius.unwrap_or(site.theme.radius),
            body: FontField {
                value: self.body_font,
                weight: self.body_weight,
                fallback: self.body_fallback,
            },
            heading: FontField {
                value: self.heading_font,
                weight: self.heading_weight,
                fallback: self.heading_fallback,
            },
            features: Features {
                news: self.feature_news.is_some(),
                chess: self.feature_chess.is_some(),
                music: self.feature_music.is_some(),
                games: self.feature_games.is_some(),
                schedule: self.feature_schedule.is_some(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormValues {
    pub identity: Identity,
    pub colors: Colors,
    pub radius: u8,
    pub body: FontField,
    pub heading: FontField,
    pub features: Features,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontField {
    pub value: String,
    pub weight: Option<u16>,
    pub fallback: FontFallback,
}

impl FontField {
    fn keeping(font: &Font) -> FontField {
        FontField {
            value: KEEP_FONT.to_string(),
            weight: Some(font.main_weight()),
            fallback: font.fallback,
        }
    }
}

impl FormValues {
    pub fn from_site(site: &SiteConfig) -> FormValues {
        FormValues {
            identity: site.identity.clone(),
            colors: site.theme.colors.clone(),
            radius: site.theme.radius,
            body: FontField::keeping(&site.theme.fonts.body),
            heading: FontField::keeping(&site.theme.fonts.heading),
            features: site.features,
        }
    }
}

pub async fn issue_url(
    site: &SiteConfig,
    admin: &AdminAuth,
    values: &FormValues,
    t: &Texts,
) -> Result<String, Vec<String>> {
    let request = build_request(site, values, &admin.fonts, t).await?;
    if !request.changes_anything(site) {
        return Err(vec![t.admin_settings_unchanged.to_string()]);
    }
    let Some(repository) = &admin.repository else {
        return Err(vec![t.admin_settings_no_repo.to_string()]);
    };
    repository.new_issue_url(&request).map_err(|e| vec![e])
}

async fn build_request(
    site: &SiteConfig,
    values: &FormValues,
    fonts: &FontCatalog,
    t: &Texts,
) -> Result<SettingsRequest, Vec<String>> {
    let mut problems = Vec::new();
    let mut resolve = async |field: &FontField, current: &Font| {
        font_request(field, current, fonts, t)
            .await
            .unwrap_or_else(|problem| {
                problems.push(problem);
                FontRequest::keep()
            })
    };
    let body = resolve(&values.body, &site.theme.fonts.body).await;
    let heading = resolve(&values.heading, &site.theme.fonts.heading).await;

    let request = SettingsRequest {
        radius: Some(values.radius),
        identity: values.identity.clone(),
        colors: values.colors.clone(),
        fonts: FontRequests { body, heading },
        features: values.features,
    };
    problems.extend(request.validate());

    if problems.is_empty() {
        Ok(request)
    } else {
        Err(problems)
    }
}

fn is_keep(value: &str) -> bool {
    let value = value.trim();
    value.is_empty() || value.eq_ignore_ascii_case(KEEP_FONT)
}

async fn font_request(
    field: &FontField,
    current: &Font,
    fonts: &FontCatalog,
    t: &Texts,
) -> Result<FontRequest, String> {
    let wanted = field.value.trim();
    if is_keep(wanted) {
        return Ok(FontRequest::Keep {
            fallback: (field.fallback != current.fallback).then_some(field.fallback),
        });
    }
    if wanted.len() > MAX_FONT_INPUT {
        return Err(fill(
            t.admin_font_unknown,
            wanted.chars().take(20).collect::<String>(),
        ));
    }

    let font = fonts
        .find(wanted)
        .await
        .map_err(|_| t.admin_font_catalog_down.to_string())?
        .ok_or_else(|| fill(t.admin_font_unknown, wanted))?;

    let weight = field
        .weight
        .or_else(|| font.closest_weight(current.main_weight()))
        .ok_or_else(|| fill(t.admin_font_unknown, wanted))?;
    if !font.has_weight(weight) {
        return Err(fill(
            &fill(t.admin_font_weight_missing, &font.family),
            weight,
        ));
    }

    Ok(FontRequest::Fontsource {
        id: font.id,
        weight,
        fallback: field.fallback,
    })
}

pub async fn render(
    app_state: &AppState,
    admin: &AdminAuth,
    username: &str,
    values: &FormValues,
    problems: &[String],
    t: &Texts,
) -> String {
    let catalog = admin.fonts.fonts().await.ok();
    let fonts = &app_state.site.theme.fonts;
    let deployed = FormValues::from_site(&app_state.site);

    html! {
        div class="flex flex-col w-full gap-8 p-6 lg:p-8 overflow-auto" {
            div class="flex flex-wrap items-center justify-between gap-4" {
                div {
                    h1 class="text-4xl text-primary-color" { (t.admin_settings_title) }
                    p class="text-sm opacity-70" { (fill(t.admin_signed_in_as, username)) }
                }
                (logout_form(t))
            }

            @if !problems.is_empty() {
                div role="alert" class="rounded-box border border-primary-color p-4" {
                    p { (t.admin_settings_errors) }
                    ul class="list-disc pl-6" {
                        @for problem in problems {
                            li { (problem) }
                        }
                    }
                }
            }

            form id="settings-form" method="post" action="/admin/settings" hx-boost="false" data-font-cdn=(FONTSOURCE_CDN_URL) class="flex flex-col gap-8 max-w-3xl" {
                (identity_section(&values.identity, &deployed.identity, t))

                section class="flex flex-col gap-4" {
                    h2 class="text-2xl" { (t.admin_colors) }
                    div class="grid grid-cols-2 md:grid-cols-3 gap-4" {
                        @for ((name, value), (_, deployed_value)) in values.colors.entries().into_iter().zip(deployed.colors.entries()) {
                            label class="flex items-center gap-2" {
                                input
                                type="color"
                                name=(format!("color_{name}"))
                                value=(value)
                                data-color=(name)
                                data-reset=(RESET_STYLES)
                                data-deployed=(deployed_value)
                                class="h-8 w-12 cursor-pointer bg-transparent";
                                span { (color_label(t, name)) }
                            }
                        }
                    }
                }

                section class="flex flex-col gap-4" {
                    h2 class="text-2xl" { (t.admin_corners) }
                    label class="flex flex-wrap items-center gap-4" {
                        span { (t.admin_radius) }
                        input
                        type="range"
                        name="radius"
                        min="0"
                        max=(MAX_RADIUS)
                        step="1"
                        value=(values.radius)
                        data-radius
                        data-reset=(RESET_STYLES)
                        data-deployed=(deployed.radius)
                        class="w-48 accent-primary-color cursor-pointer";
                        output id="radius-value" class="tabular-nums" { (values.radius) "px" }
                    }
                }

                section class="flex flex-col gap-4" {
                    h2 class="text-2xl" { (t.admin_fonts) }
                    p data-font-hint class="text-sm opacity-70" { (t.admin_font_hint) }
                    @if catalog.is_none() {
                        p role="status" class="text-primary-color" { (t.admin_font_catalog_down) }
                    }
                    (font_fieldset("body", t.admin_font_body, &fonts.body, &values.body, &deployed.body, t))
                    (font_fieldset("heading", t.admin_font_heading, &fonts.heading, &values.heading, &deployed.heading, t))
                    datalist id="font-catalog" {
                        option value=(KEEP_FONT) { (t.admin_font_keep) }
                        @if let Some(catalog) = &catalog {
                            @for font in catalog.iter() {
                                option value=(font.family) {
                                    (if font.is_google() { t.admin_font_source_google } else { t.admin_font_source_open })
                                }
                            }
                        }
                    }
                }

                section class="flex flex-col gap-4" {
                    h2 class="text-2xl" { (t.admin_features) }
                    div class="flex flex-wrap gap-4" {
                        @for (((name, enabled), (_, active)), (_, deployed_enabled)) in values.features.entries().into_iter().zip(app_state.features.entries()).zip(deployed.features.entries()) {
                            label class="flex items-center gap-2 cursor-pointer" {
                                input
                                type="checkbox"
                                name=(format!("feature_{name}"))
                                checked[enabled]
                                data-reset=(RESET_DETAILS)
                                data-deployed=(deployed_enabled);
                                span { (feature_label(t, name)) }
                                @if enabled && !active {
                                    span class="text-sm text-primary-color" { "(" (t.admin_feature_inactive) ")" }
                                }
                            }
                        }
                    }
                }

                (preview_section(fonts, t))

                div class="flex flex-col gap-2" {
                    p class="text-sm opacity-70" { (t.admin_reset_hint) }
                    div class="flex flex-wrap gap-4" {
                        (reset_button(RESET_STYLES, t.admin_reset_styles))
                        (reset_button(RESET_DETAILS, t.admin_reset_details))
                    }
                }

                div class="flex flex-col gap-2" {
                    p class="text-sm opacity-70" { (t.admin_submit_hint) }
                    button
                    type="submit"
                    class="rounded-box self-start border border-primary-color px-3 py-1 cursor-pointer hover:bg-primary-color hover:text-background-color"
                    { (t.admin_submit) }
                }
            }

            script type="application/json" id="font-catalog-data" {
                (PreEscaped(catalog_json(catalog.as_deref().map(Vec::as_slice).unwrap_or_default())))
            }
            script src="/_static/admin-settings.js" {}
        }
    }
    .into_string()
}

fn preview_section(fonts: &Fonts, t: &Texts) -> Markup {
    html! {
        section id="settings-preview" class="rounded-box flex flex-col gap-4 border border-shade-color p-4" {
            h2 class="text-sm uppercase opacity-70" { (t.admin_preview) }
            label class="flex flex-col gap-1" {
                span class="text-sm" { (t.admin_preview_sample) }
                input
                type="text"
                data-preview-sample
                value=(t.admin_preview_heading)
                maxlength="120"
                autocomplete="off"
                class=(INPUT_CLASS);
            }
            h3 data-sample-text=(t.admin_preview_heading) class="text-3xl text-primary-color break-words" { (t.admin_preview_heading) }
            div class="prose prose-theme max-w-none" {
                p { (t.admin_preview_body) }
                p {
                    a href="#settings-preview" { (t.admin_preview_link) } ", "
                    strong { (t.admin_preview_bold) } ", "
                    em { (t.admin_preview_italic) } ", "
                    code { (t.admin_preview_code) }
                }
            }
            @for (role, label, font) in [
                ("body", t.admin_font_body, &fonts.body),
                ("heading", t.admin_font_heading, &fonts.heading),
            ] {
                div class="flex flex-col gap-1" {
                    p class="text-sm opacity-70" { (label) " · " (t.admin_preview_weights) }
                    div
                    data-specimen=(role)
                    data-kept-weights=(kept_weights(font))
                    data-kept-weight=(font.main_weight())
                    class="flex flex-wrap gap-3" {}
                    p
                    data-font-status=(role)
                    data-loading=(t.admin_preview_loading)
                    data-failed=(t.admin_preview_failed)
                    aria-live="polite"
                    class="text-sm text-primary-color" {}
                }
            }
            div class="rounded-box flex flex-col gap-3 border border-primary-color bg-shade-color p-4" {
                h4 data-sample-text=(t.admin_preview_heading) class="text-xl break-words" { (t.admin_preview_heading) }
                p class="text-sm" { (t.admin_preview_body) }
                div class="flex flex-wrap items-center gap-3 text-sm" {
                    span class="rounded-box bg-primary-color px-3 py-1 text-background-color" { (t.admin_preview_button) }
                    span class="rounded-box border border-secondary-color px-2 text-secondary-color" { (t.admin_preview_tag) }
                    span class="text-accent-color" { (t.admin_color_accent) }
                    span class="rounded-box bg-background-color px-2" { (t.admin_color_background) }
                }
            }
        }
    }
}

fn kept_weights(font: &Font) -> String {
    let mut weights: Vec<u16> = font
        .faces
        .iter()
        .filter(|face| face.style == FontStyle::Normal)
        .map(|face| face.weight)
        .collect();
    if weights.is_empty() {
        weights = font.faces.iter().map(|face| face.weight).collect();
    }
    weights.sort_unstable();
    weights.dedup();
    weights
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

fn reset_button(group: &str, label: &str) -> Markup {
    html! {
        button type="button" data-reset-group=(group) class=(RESET_BUTTON_CLASS) { (label) }
    }
}

fn identity_section(identity: &Identity, deployed: &Identity, t: &Texts) -> Markup {
    let field = |label: &str, input: Markup| {
        html! {
            label class="flex flex-col gap-1" {
                span class="text-sm" { (label) }
                (input)
            }
        }
    };

    html! {
        section class="flex flex-col gap-4" {
            h2 class="text-2xl" { (t.admin_identity) }
            (field(t.admin_site_name, html! {
                input type="text" name="site_name" value=(identity.name) data-reset=(RESET_DETAILS) data-deployed=(deployed.name) required maxlength="80" class=(INPUT_CLASS);
            }))
            @for locale in Locale::ALL {
                (field(&fill(t.admin_site_description, locale.as_str()), html! {
                    input
                    type="text"
                    name=(format!("description_{}", locale.as_str()))
                    value=[identity.description.get(locale.as_str())]
                    data-reset=(RESET_DETAILS)
                    data-deployed=(deployed.description.get(locale.as_str()).map(String::as_str).unwrap_or_default())
                    maxlength="300"
                    class=(INPUT_CLASS);
                }))
            }
            (field(t.admin_source_url, html! {
                input type="url" name="site_source_url" value=[identity.source_url.as_deref()] data-reset=(RESET_DETAILS) data-deployed=(deployed.source_url.as_deref().unwrap_or_default()) maxlength="300" class=(INPUT_CLASS);
            }))
            (field(t.admin_links, html! {
                textarea name="site_links" rows="4" spellcheck="false" data-reset=(RESET_DETAILS) data-deployed=(links_text(&deployed.links)) class=(INPUT_CLASS) { (links_text(&identity.links)) }
            }))
            p class="text-sm opacity-70" { (t.admin_links_hint) }
        }
    }
}

fn parse_links(text: &str) -> Vec<Link> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| match line.rsplit_once(char::is_whitespace) {
            Some((label, url)) => Link {
                label: label.trim().to_string(),
                url: url.to_string(),
            },
            None => Link {
                label: String::new(),
                url: line.to_string(),
            },
        })
        .collect()
}

fn links_text(links: &[Link]) -> String {
    links
        .iter()
        .map(|link| format!("{} {}", link.label, link.url))
        .collect::<Vec<_>>()
        .join("\n")
}

fn font_fieldset(
    role: &str,
    label: &str,
    current: &Font,
    field: &FontField,
    deployed: &FontField,
    t: &Texts,
) -> Markup {
    html! {
        fieldset class="rounded-box flex flex-col gap-2 border border-shade-color p-4" {
            legend class="px-1" { (label) }
            p class="text-sm opacity-70" { (fill(t.admin_font_current, &current.family)) }
            input
            type="text"
            name=(format!("{role}_font"))
            value=(field.value)
            list="font-catalog"
            data-font-role=(role)
            data-reset=(RESET_STYLES)
            data-deployed=(deployed.value)
            autocomplete="off"
            spellcheck="false"
            maxlength=(MAX_FONT_INPUT)
            class=(INPUT_CLASS);
            (font_picker(role, label, current, t))
            div class="flex flex-wrap gap-4" {
                label class="flex flex-col gap-1" {
                    span class="text-sm" { (t.admin_font_weight) }
                    select name=(format!("{role}_weight")) data-font-role=(role) data-reset=(RESET_STYLES) data-deployed=[deployed.weight] disabled[is_keep(&field.value)] class=(INPUT_CLASS) {
                        @for weight in WEIGHTS {
                            option value=(weight) selected[Some(weight) == field.weight] { (weight) }
                        }
                    }
                }
                label class="flex flex-col gap-1" {
                    span class="text-sm" { (t.admin_font_fallback) }
                    select name=(format!("{role}_fallback")) data-font-role=(role) data-reset=(RESET_STYLES) data-deployed=(deployed.fallback.as_css()) class=(INPUT_CLASS) {
                        @for fallback in FontFallback::ALL {
                            option value=(fallback.as_css()) selected[fallback == field.fallback] {
                                (fallback.as_css())
                            }
                        }
                    }
                }
            }
        }
    }
}

fn font_picker(role: &str, label: &str, current: &Font, t: &Texts) -> Markup {
    let list_id = format!("{role}-font-list");
    html! {
        div
        data-font-picker=(role)
        data-keep-label=(fill(t.admin_font_keep_current, &current.family))
        data-weights-label=(t.admin_font_weights)
        hidden
        class="relative flex flex-col" {
            button
            type="button"
            data-picker-toggle
            aria-haspopup="listbox"
            aria-expanded="false"
            aria-label=(fill(t.admin_font_choose, label))
            class=(format!("{INPUT_CLASS} flex items-center justify-between gap-2 text-left cursor-pointer")) {
                span data-picker-label class="truncate text-lg" {}
                span aria-hidden="true" class="opacity-70" { "▾" }
            }
            div
            data-picker-panel
            hidden
            class="rounded-box absolute top-full left-0 right-0 z-20 mt-1 flex flex-col gap-2 border border-shade-color bg-background-color p-2 shadow-lg" {
                input
                type="search"
                data-picker-search
                role="combobox"
                aria-expanded="true"
                aria-controls=(list_id)
                aria-autocomplete="list"
                aria-label=(t.admin_font_search)
                placeholder=(t.admin_font_search)
                autocomplete="off"
                spellcheck="false"
                class=(INPUT_CLASS);
                div role="group" aria-label=(t.admin_font_categories) class="flex flex-wrap gap-1" {
                    @for (category, name) in font_categories(t) {
                        button
                        type="button"
                        data-picker-category=(category)
                        aria-pressed=(category.is_empty())
                        class="rounded-box border border-shade-color px-2 text-sm cursor-pointer aria-pressed:border-primary-color aria-pressed:text-primary-color" { (name) }
                    }
                }
                ul id=(list_id) role="listbox" aria-label=(label) class="max-h-72 overflow-auto flex flex-col" {}
                p data-picker-empty hidden class="text-sm opacity-70 px-2" { (t.admin_font_no_match) }
            }
        }
    }
}

fn font_categories(t: &Texts) -> [(&'static str, &str); 6] {
    [
        ("", t.admin_font_category_all),
        ("sans-serif", t.admin_font_category_sans_serif),
        ("serif", t.admin_font_category_serif),
        ("display", t.admin_font_category_display),
        ("handwriting", t.admin_font_category_handwriting),
        ("monospace", t.admin_font_category_monospace),
    ]
}

fn color_label<'a>(t: &'a Texts, name: &str) -> &'a str {
    match name {
        "background" => t.admin_color_background,
        "bright" => t.admin_color_bright,
        "primary" => t.admin_color_primary,
        "secondary" => t.admin_color_secondary,
        "accent" => t.admin_color_accent,
        _ => t.admin_color_shade,
    }
}

fn feature_label<'a>(t: &'a Texts, name: &str) -> &'a str {
    match name {
        "news" => t.nav_news,
        "chess" => t.nav_chess,
        "music" => t.nav_music,
        "games" => t.nav_games,
        _ => t.admin_feature_schedule,
    }
}

fn catalog_json(fonts: &[CatalogFont]) -> String {
    serde_json::to_string(fonts)
        .unwrap_or_else(|_| "[]".to_string())
        .replace('<', "\\u003c")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Locale;
    use crate::site_config::STATIC_DIR;

    const SITE_CONFIG_PATH: &str = "crates/e2e/fixtures/site.toml";

    fn site() -> SiteConfig {
        SiteConfig::load(SITE_CONFIG_PATH, STATIC_DIR).unwrap()
    }

    fn form(feature_chess: Option<&str>) -> SettingsForm {
        SettingsForm {
            site_name: "  Ana's garden ".to_string(),
            site_source_url: " ".to_string(),
            description_en: "Notes".to_string(),
            description_es: String::new(),
            site_links: "MY GITHUB https://github.com/ana\r\n\r\nbroken-line\n".to_string(),
            color_background: "#000000".to_string(),
            color_bright: "#ffffff".to_string(),
            color_primary: "#ff0000".to_string(),
            color_secondary: "#00ff00".to_string(),
            color_accent: "#0000ff".to_string(),
            color_shade: "#111111".to_string(),
            radius: Some(10),
            body_font: "Inter".to_string(),
            body_weight: Some(600),
            body_fallback: FontFallback::SansSerif,
            heading_font: KEEP_FONT.to_string(),
            heading_weight: Some(400),
            heading_fallback: FontFallback::Serif,
            feature_news: Some("on".to_string()),
            feature_chess: feature_chess.map(str::to_string),
            feature_music: Some("on".to_string()),
            feature_games: None,
            feature_schedule: Some("on".to_string()),
        }
    }

    #[test]
    fn the_form_starts_from_the_current_site() {
        let site = site();
        let values = FormValues::from_site(&site);

        assert_eq!(values.colors, site.theme.colors);
        assert_eq!(values.body.value, KEEP_FONT);
        assert_eq!(values.body.weight, Some(600));
        assert_eq!(values.heading.fallback, FontFallback::SansSerif);
        assert_eq!(values.features, site.features);
        assert_eq!(values.radius, site.theme.radius);
    }

    #[test]
    fn the_radius_comes_from_the_form_or_stays_as_it_is() {
        let mut site = site();
        site.theme.radius = 4;
        assert_eq!(form(None).into_values(&site).radius, 10);

        let without = SettingsForm {
            radius: None,
            ..form(None)
        };
        assert_eq!(without.into_values(&site).radius, 4);
    }

    #[test]
    fn unchecked_boxes_switch_features_off() {
        let values = form(None).into_values(&site());

        assert!(!values.features.chess && !values.features.games);
        assert!(values.features.news && values.features.music && values.features.schedule);
        assert!(form(Some("on")).into_values(&site()).features.chess);
        assert_eq!(values.body.value, "Inter");
        assert_eq!(values.colors.primary, "#ff0000");
    }

    #[test]
    fn the_identity_is_read_from_the_form() {
        let mut site = site();
        site.identity
            .description
            .insert("pt".to_string(), "Notas".to_string());
        let identity = form(None).into_values(&site).identity;

        assert_eq!(identity.name, "Ana's garden");
        assert_eq!(identity.source_url, None);
        assert_eq!(identity.description["en"], "Notes");
        assert!(!identity.description.contains_key("es"));
        assert_eq!(identity.description["pt"], "Notas");
        assert_eq!(
            identity.links,
            vec![
                Link {
                    label: "MY GITHUB".to_string(),
                    url: "https://github.com/ana".to_string(),
                },
                Link {
                    label: String::new(),
                    url: "broken-line".to_string(),
                },
            ]
        );
    }

    #[test]
    fn links_round_trip_through_the_textarea() {
        let links = site().identity.links;

        assert_eq!(parse_links(&links_text(&links)), links);
        assert_eq!(
            links_text(&links),
            "GITHUB https://www.github.com/Chemchu\nLINKEDIN https://www.linkedin.com/in/gustavo-lee"
        );
    }

    #[test]
    fn every_locale_has_a_description_field() {
        let langs: Vec<&str> = Locale::ALL.iter().map(|locale| locale.as_str()).collect();
        assert_eq!(langs, vec!["en", "es"]);

        let site = site();
        let html =
            identity_section(&site.identity, &site.identity, Locale::En.texts()).into_string();
        assert!(html.contains(r#"name="description_en" value="My personal digital garden"#));
        assert!(html.contains(r#"name="description_es""#));
        assert!(
            html.contains(r#"name="site_source_url" value="https://github.com/Chemchu/guslee""#)
        );
    }

    #[test]
    fn every_color_and_feature_has_a_label() {
        let t = Locale::Es.texts();

        let labels: Vec<&str> = site()
            .theme
            .colors
            .entries()
            .map(|(name, _)| color_label(t, name))
            .to_vec();
        assert_eq!(labels.len(), 6);
        assert_eq!(
            labels
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            6
        );
        let features: Vec<&str> = Features::default()
            .entries()
            .iter()
            .map(|(name, _)| feature_label(t, name))
            .collect();
        assert_eq!(
            features,
            vec!["NOVEDADES", "AJEDREZ", "MÚSICA", "JUEGOS", "Horario"]
        );
    }

    #[test]
    fn the_catalog_json_cannot_close_its_script_tag() {
        let fonts = vec![CatalogFont {
            id: "x".to_string(),
            family: "</script><script>alert(1)</script>".to_string(),
            weights: vec![400],
            category: "serif".to_string(),
            subset: "khmer".to_string(),
            kind: "google".to_string(),
        }];

        let json = catalog_json(&fonts);
        assert!(json.contains(r#""subset":"khmer""#) && !json.contains("defSubset"));
        assert!(!json.contains('<'));
        assert!(!json.contains("\"kind\"") && !json.contains("\"type\""));
        assert_eq!(catalog_json(&[]), "[]");
    }

    #[test]
    fn font_fieldsets_offer_every_weight_and_fallback() {
        let site = site();
        let field = FontField {
            value: "\"><b>".to_string(),
            weight: Some(300),
            fallback: FontFallback::Monospace,
        };
        let html = font_fieldset(
            "body",
            "Body",
            &site.theme.fonts.body,
            &field,
            &FontField::keeping(&site.theme.fonts.body),
            Locale::En.texts(),
        )
        .into_string();

        assert_eq!(html.matches("<option").count(), 9 + 5);
        assert!(html.contains(r#"<option value="300" selected>"#));
        assert!(html.contains(r#"<option value="monospace" selected>"#));
        assert!(html.contains("Current: Iosevka"));
        assert!(html.contains(r#"value="&quot;&gt;&lt;b&gt;""#));
        assert!(!html.contains("disabled"));
    }

    #[test]
    fn font_pickers_start_hidden_until_the_script_enhances_them() {
        let mut current = site().theme.fonts.body;
        current.family = "Ana's <Font>".to_string();
        let html = font_picker("body", "Body text", &current, Locale::En.texts()).into_string();

        assert!(html.contains(r#"data-font-picker="body""#));
        assert!(html.contains(r#"data-keep-label="Keep the current font (Ana's &lt;Font&gt;)""#));
        assert!(html.contains(r#"data-weights-label="{} weights" hidden"#));
        assert!(html.contains(r#"aria-label="Body text: choose a font""#));
        assert!(html.contains(r#"aria-controls="body-font-list""#));
        assert!(html.contains(r#"<ul id="body-font-list" role="listbox" aria-label="Body text""#));
        assert!(html.contains(r#"data-picker-category="" aria-pressed="true""#));
        for category in ["sans-serif", "serif", "display", "handwriting", "monospace"] {
            assert!(
                html.contains(&format!(
                    r#"data-picker-category="{category}" aria-pressed="false""#
                )),
                "{category}"
            );
        }
        assert!(!html.contains("name="));
    }

    #[test]
    fn every_locale_names_the_font_categories() {
        for locale in Locale::ALL {
            let names: Vec<&str> = font_categories(locale.texts())
                .iter()
                .map(|(_, name)| *name)
                .collect();
            assert!(names.iter().all(|name| !name.is_empty()), "{names:?}");
        }
    }

    #[test]
    fn kept_weights_list_the_upright_faces_once() {
        let face = |weight, style| crate::site_config::FontFace {
            src: "fonts/x/x.woff2".to_string(),
            weight,
            style,
            unicode_range: None,
        };
        let mut font = site().theme.fonts.body;
        font.faces = vec![
            face(700, FontStyle::Normal),
            face(400, FontStyle::Italic),
            face(400, FontStyle::Normal),
            face(700, FontStyle::Normal),
        ];
        assert_eq!(kept_weights(&font), "400 700");

        font.faces = vec![face(300, FontStyle::Italic)];
        assert_eq!(kept_weights(&font), "300");
    }

    #[test]
    fn the_preview_shows_samples_specimens_and_a_card() {
        let site = site();
        let html = preview_section(&site.theme.fonts, Locale::En.texts()).into_string();

        assert!(
            html.contains(r#"<input type="text" data-preview-sample value="The quick brown fox""#)
        );
        assert_eq!(
            html.matches(r#"data-sample-text="The quick brown fox""#)
                .count(),
            2
        );
        assert!(html.contains("<strong>bold text</strong>"));
        assert!(html.contains("<code>inline code</code>"));
        for role in ["body", "heading"] {
            assert!(html.contains(&format!(
                r#"data-specimen="{role}" data-kept-weights="600" data-kept-weight="600""#
            )));
            assert!(html.contains(&format!(
                r#"data-font-status="{role}" data-loading="Loading {{}}…""#
            )));
        }
        assert!(html.contains(r#"aria-live="polite""#));
        assert!(!html.contains("name="));
    }

    #[test]
    fn kept_fonts_have_no_weight_to_choose() {
        let site = site();
        let html = font_fieldset(
            "body",
            "Body",
            &site.theme.fonts.body,
            &FontField::keeping(&site.theme.fonts.body),
            &FontField::keeping(&site.theme.fonts.body),
            Locale::En.texts(),
        )
        .into_string();

        assert!(html.contains(r#"data-deployed="600" disabled"#));
        assert!(is_keep(" KEEP ") && is_keep("") && !is_keep("Inter"));
    }
}
