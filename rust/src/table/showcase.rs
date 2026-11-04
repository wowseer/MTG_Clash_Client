//! `Showcase`: Bühne vor der Kamera für Karten, über die der Spieler entscheidet (Baustein der
//! 3D-Oberfläche; zuerst für den Mulligan).
//!
//! Aufbau im Koordinatensystem der Kamera (x = rechts, y = oben, z = zur Kamera), in festem
//! Abstand vor ihr:
//! - **Abdunkelung**: dunkle, halbtransparente Fläche hinter allem (blendet den Tisch ab),
//! - **Kartenfächer**: eine echte [`Zone`] (Art Hand, flach und breit), in die Karten von ihrem
//!   Platz auf dem Tisch hineingleiten und aus der sie wieder zurückgehen,
//! - **Titel / Untertitel / Hinweis**: `Label3D`,
//! - **Knöpfe**: [`Button3D`] in einer Reihe unter dem Fächer,
//! - **Schieberegler** ([`Showcase::enable_scroll`]): bei breiten Fächern ein [`Slider3D`]
//!   zwischen Fächer und Knöpfen,
//! - **Optionen** ([`Showcase::show_options`]): eigene Karten (z. B. Kopien je Spielweise einer
//!   Karte) mit Beschriftung darüber, die die Bühne beim Schließen freigibt,
//! - **Cover-Flow** ([`Showcase::enable_coverflow`]): großer Fächer zum Durchsuchen vieler Karten
//!   (Bibliothek), die Karte unter der Maus kommt groß und gerade nach vorn,
//! - **Schlachtfeld ansehen** ([`Showcase::enable_peek`]): Knopf oben rechts, der die Bühne
//!   ausblendet, bis man ihn noch einmal drückt,
//! - **Sperre**: solange abgedunkelt ist, lassen sich Karten auf dem Tisch dahinter nicht hovern.
//!
//! Zeiten nach `docs/claude/ANIMATION.md`: Öffnen 0,20 s (Abdunkeln 0,15 s), Schließen 0,15 s.

use godot::classes::base_material_3d::{ShadingMode, Transparency};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::label_3d::DrawFlags;
use godot::classes::{Area3D, BoxShape3D, CollisionObject3D, CollisionShape3D, INode3D, Label3D, MeshInstance3D, QuadMesh, StandardMaterial3D};
use godot::prelude::*;

use super::button3d::{Button3D, ButtonStyle};
use super::layers::CARD_LAYER;
use super::slider3d::Slider3D;
use super::card3d::{Card3D, CARD_HEIGHT};
use super::zone::{Zone, ZoneKind};

/// Abstand der Bühne vor der Kamera (Welteinheiten).
const DISTANCE: f32 = 4.5;
/// Deckkraft der Abdunkelung und ihre Ein-/Ausblendzeit; Abstand der Abdunkelung (und der
/// Sperre für Tischkarten) hinter der Bühnenebene – hinter den hintersten Cover-Flow-Karten.
const DIM_ALPHA: f32 = 0.62;
const DIM_TIME: f32 = 0.15;
const DIM_DEPTH: f32 = 1.6;
const DIM_SIZE: Vector2 = Vector2::new(16.0, 9.0);
/// Cover-Flow ([`Showcase::enable_coverflow`]): Höhe der Fächermitte, Lage des Schiebereglers,
/// Breite, ab der er erscheint, und Größe der Karte unter der Maus (Anteil der Bildhöhe).
const COVERFLOW_CENTER_Y: f32 = 0.1;
const COVERFLOW_SLIDER_Y: f32 = -0.66;
const COVERFLOW_SCROLL_MIN: f32 = 3.0;
const COVERFLOW_ZOOM_HEIGHT: f32 = 0.56;
/// „Schlachtfeld ansehen“: Lage und Breite des Knopfes (oben rechts neben dem Titel).
const PEEK_POS: Vector2 = Vector2::new(1.62, 0.93);
const PEEK_WIDTH: f32 = 0.86;
/// Kartenfächer: Mitte (lokal), Radius und Abstand der Karten (sehr flacher Bogen).
const FAN_CENTER: Vector3 = Vector3::new(0.0, 0.18, 0.0);
const FAN_RADIUS: f32 = 10.0;
const FAN_STEP: f32 = 0.58;
/// Knopfreihe: Höhe (lokal), Breite je Knopf und Abstand der Knopfmitten.
const BUTTON_ROW_Y: f32 = -0.72;
const BUTTON_WIDTH: f32 = 0.95;
const BUTTON_GAP: f32 = 0.92;
/// Abstand der Options-Beschriftung über der Kartenoberkante.
const CAPTION_GAP: f32 = 0.07;
/// Schiebbarer Fächer ([`Showcase::enable_scroll`]): Kartenabstand (nebeneinander, ohne
/// Überlappung), Bogenradius (fast gerade), sichtbare Breite und Lage/Länge des Schiebereglers.
const SCROLL_STEP: f32 = 0.7;
const SCROLL_RADIUS: f32 = 30.0;
const SCROLL_SPAN: f32 = 3.4;
const SLIDER_Y: f32 = -0.42;
const SLIDER_WIDTH: f32 = 3.0;
/// Fokus-Anordnung ([`Showcase::set_focus_layout`]) für Fragen zu einer Karte (Kicker, Ja/Nein):
/// Karte groß in der Mitte (Größe, Lage), Knöpfe untereinander darunter (oberster Knopf,
/// Abstand, Breite).
const FOCUS_SCALE: f32 = 1.3;
const FOCUS_CENTER: Vector3 = Vector3::new(0.0, 0.3, 0.0);
const FOCUS_BUTTON_TOP: f32 = -0.5;
const FOCUS_STEP: f32 = 0.3;
const FOCUS_BUTTON_WIDTH: f32 = 1.3;

/// Bühne vor der Kamera: Abdunkelung, Kartenfächer ([Zone]), Titel und [Button3D]-Knöpfe.
/// [method open] stellt sie vor die aktuelle Kamera, [method show_cards] holt Karten herein,
/// [method close] gibt sie an ihre Zonen zurück und blendet aus.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct Showcase {
    zone: Option<Gd<Zone>>,
    dimmer: Option<Gd<StandardMaterial3D>>,
    title: Option<Gd<Label3D>>,
    subtitle: Option<Gd<Label3D>>,
    note: Option<Gd<Label3D>>,
    buttons: Vec<Gd<Button3D>>,
    /// Schiebbarer Fächer: Überbreite (Welteinheiten) und Regler.
    scroll_extra: f32,
    slider: Option<Gd<Slider3D>>,
    /// Fokus-Anordnung: Karte groß, Knöpfe untereinander darunter statt in einer Reihe.
    focus: bool,
    /// Fächer als Cover-Flow: Schieben verschiebt die Karten statt der Zone.
    coverflow: bool,
    /// Sperre hinter der Bühne (Kartenlayer): Tischkarten dahinter bekommen keinen Hover.
    blocker: Option<Gd<Area3D>>,
    /// „Schlachtfeld ansehen“: Knopf und – solange die Bühne ausgeblendet ist – je Kind der
    /// Bühne Sichtbarkeit und Kollisionslayer vor dem Ausblenden.
    peek_button: Option<Gd<Button3D>>,
    peek_saved: Vec<(Gd<Node3D>, bool, Option<u32>)>,
    /// Karten und die Zone, aus der sie kamen (für die Rückgabe).
    borrowed: Vec<(Gd<Card3D>, Gd<Zone>)>,
    /// Eigene Karten der Bühne (z. B. Kopien als Optionen); werden beim Schließen freigegeben.
    owned: Vec<Gd<Card3D>>,
    /// Karten, die während der Bühne ausgeblendet sind (z. B. das Original hinter einer Kopie);
    /// erscheinen wieder, wenn die Bühne ganz verschwunden ist.
    hidden: Vec<Gd<Card3D>>,
    /// Zeit seit dem Öffnen bzw. Beginn des Schließens.
    t: f32,
    closing: bool,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for Showcase {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            zone: None,
            dimmer: None,
            title: None,
            subtitle: None,
            note: None,
            buttons: Vec::new(),
            scroll_extra: 0.0,
            slider: None,
            focus: false,
            coverflow: false,
            blocker: None,
            peek_button: None,
            peek_saved: Vec::new(),
            borrowed: Vec::new(),
            owned: Vec::new(),
            hidden: Vec::new(),
            t: 0.0,
            closing: false,
            base,
        }
    }

    fn ready(&mut self) {
        self.build();
    }

    fn process(&mut self, delta: f64) {
        self.t += delta as f32;
        let k = (self.t / DIM_TIME).min(1.0);
        let peeking = !self.peek_saved.is_empty();
        let alpha = if self.closing { DIM_ALPHA * (1.0 - k) } else if peeking { 0.0 } else { DIM_ALPHA * k };
        if let Some(m) = self.dimmer.as_mut() {
            m.set_albedo(Color::from_rgba(0.02, 0.02, 0.04, alpha));
        }
        let text_alpha = if self.closing { 1.0 - k } else { k };
        for l in [&mut self.title, &mut self.subtitle, &mut self.note].into_iter().flatten() {
            let mut c = l.get_modulate();
            c.a = text_alpha;
            l.set_modulate(c);
        }
        if self.closing && k >= 1.0 {
            // ausgeblendete Karten erst zeigen, wenn die Bühne ganz verschwunden ist
            for mut c in self.hidden.drain(..).filter(|c| c.is_instance_valid()) {
                c.set_visible(true);
            }
            self.base_mut().queue_free();
        }
    }
}

#[godot_api]
impl Showcase {
    /// Die Bühne an einer Stelle auf dem Tisch statt vor der Kamera (z. B. am Stack), parallel
    /// zur Bildebene, um `scale` vergrößert, ohne Abdunkelung: `point` = Weltpunkt der
    /// Bühnenmitte, die Bühne rückt `lift` zur Kamera hin (vor die Karten dort).
    #[func]
    pub fn open_at(&mut self, point: Vector3, scale: f32, lift: f32) {
        let Some(cam) = self.base().get_viewport().and_then(|v| v.get_camera_3d()) else { return };
        let basis = cam.get_global_transform().basis.orthonormalized();
        let toward_cam = basis.col_c();
        self.base_mut().set_global_transform(Transform3D::new(basis.scaled(Vector3::splat(scale)), point + toward_cam * lift));
        // ohne Abdunkelung: der Tisch bleibt sichtbar und bedienbar
        self.dimmer = None;
        if let Some(mut b) = self.blocker.take() {
            b.queue_free();
        }
        if let Some(mut n) = self.base().get_node_or_null("Dimmer").and_then(|n| n.try_cast::<Node3D>().ok()) {
            n.set_visible(false);
        }
        self.t = 0.0;
        self.closing = false;
    }

    /// Stellt die Bühne vor die aktuelle Kamera (Abstand fest, parallel zur Bildebene).
    #[func]
    pub fn open(&mut self) {
        let Some(cam) = self.base().get_viewport().and_then(|v| v.get_camera_3d()) else { return };
        let xf = cam.get_global_transform();
        let forward = -xf.basis.col_c();
        let basis = xf.basis.orthonormalized();
        self.base_mut().set_global_transform(Transform3D::new(basis, xf.origin + forward * DISTANCE));
        self.t = 0.0;
        self.closing = false;
    }

    /// Titel, Untertitel und Hinweis setzen (leer = ausblenden).
    #[func]
    pub fn set_texts(&mut self, title: GString, subtitle: GString, note: GString) {
        for (label, text) in [(&mut self.title, title), (&mut self.subtitle, subtitle), (&mut self.note, note)] {
            if let Some(l) = label.as_mut() {
                l.set_text(&text);
                l.set_visible(!text.is_empty());
            }
        }
    }

    /// Karten in den Fächer holen (sie gleiten von ihrem Platz herein); beim Schließen gehen sie
    /// in ihre bisherige Zone zurück.
    pub fn show_cards(&mut self, cards: &[Gd<Card3D>]) {
        let Some(mut zone) = self.zone.clone() else { return };
        for card in cards {
            let Some(mut from) = card.bind().get_zone() else { continue };
            if from == zone {
                continue;
            }
            from.bind_mut().remove_card(card.clone());
            self.borrowed.push((card.clone(), from));
            card.clone().bind_mut().set_bright(true);
            zone.bind_mut().add_card(card.clone(), -1, 0.0);
        }
    }

    /// Eigene Karten als Optionen in den Fächer legen (z. B. Kopien einer Karte je Spielweise),
    /// je mit Beschriftung darüber (leer = keine). Die Bühne gibt sie beim Schließen frei.
    pub fn show_options(&mut self, options: Vec<(Gd<Card3D>, String)>) {
        let Some(mut zone) = self.zone.clone() else { return };
        let start = zone.get_global_transform();
        for (mut card, caption) in options {
            // in den Baum (unter die Bühne), aus der Fächermitte heraus
            self.base_mut().add_child(&card);
            card.set_global_transform(start);
            card.bind_mut().set_bright(true);
            zone.bind_mut().add_card(card.clone(), -1, 0.0);
            if !caption.is_empty() {
                let mut l = Label3D::new_alloc();
                l.set_text(&caption);
                l.set_font_size(64);
                l.set_pixel_size(0.0016);
                l.set_outline_size(12);
                l.set_modulate(Color::from_rgb(0.93, 0.95, 1.0));
                l.set_outline_modulate(Color::from_rgba(0.02, 0.03, 0.08, 0.9));
                l.set_draw_flag(DrawFlags::SHADED, false);
                l.set_draw_flag(DrawFlags::DOUBLE_SIDED, false);
                // flach auf der Kartenebene, über der Oberkante (Karte: Vorderseite +Y, oben −Z)
                l.set_transform(Transform3D::new(
                    Basis::from_axis_angle(Vector3::RIGHT, -std::f32::consts::FRAC_PI_2),
                    Vector3::new(0.0, 0.01, -CARD_HEIGHT * 0.5 - CAPTION_GAP),
                ));
                card.add_child(&l);
            }
            self.owned.push(card);
        }
    }

    /// Knopf in die Knopfreihe unter dem Fächer setzen (Reihenfolge = links nach rechts).
    pub fn add_button(&mut self, button: Gd<Button3D>) {
        self.base_mut().add_child(&button);
        self.buttons.push(button.clone());
        let n = self.buttons.len() as f32;
        let focus = self.focus;
        for (i, b) in self.buttons.iter_mut().enumerate() {
            let pos = if focus {
                Vector3::new(0.0, FOCUS_BUTTON_TOP - i as f32 * FOCUS_STEP, 0.05)
            } else {
                Vector3::new((i as f32 - (n - 1.0) * 0.5) * BUTTON_GAP, BUTTON_ROW_Y, 0.05)
            };
            b.set_position(pos);
        }
    }

    /// Karte ausblenden, solange die Bühne offen ist (z. B. das Original auf dem Stack hinter
    /// seiner großen Kopie); sie erscheint wieder, sobald die Bühne nach dem Schließen ganz
    /// ausgeblendet ist.
    pub fn hide_while_open(&mut self, card: Gd<Card3D>) {
        let mut card = card;
        card.set_visible(false);
        self.hidden.push(card);
    }

    /// Knopf an eine feste Stelle der Bühne setzen (lokal x/y, z. B. eine waagerechte
    /// Knopfreihe unter der Karte); wird wie die anderen Knöpfe beim Schließen ausgeblendet.
    pub fn add_button_at(&mut self, button: Gd<Button3D>, pos: Vector2) {
        let mut button = button;
        self.base_mut().add_child(&button);
        button.set_position(Vector3::new(pos.x, pos.y, 0.05));
        self.buttons.push(button);
    }

    /// Fokus-Anordnung für Fragen zu einer Karte (Kicker, Ja/Nein …): die Karte groß und hell
    /// in der Mitte, die Knöpfe untereinander darunter, kein Untertitel (er läge auf der Karte).
    /// Vor [method add_button] aufrufen.
    #[func]
    pub fn set_focus_layout(&mut self) {
        self.focus = true;
        if let Some(mut zone) = self.zone.clone() {
            let mut xf = zone.get_transform();
            xf.origin = FOCUS_CENTER;
            zone.set_transform(xf);
            zone.set("card_scale", &FOCUS_SCALE.to_variant());
            zone.bind_mut().layout();
        }
        if let Some(l) = self.subtitle.as_mut() {
            l.set_visible(false);
        }
    }

    /// Breite eines Knopfes in der Fokus-Anordnung (Welteinheiten).
    #[func]
    pub fn focus_button_width() -> f32 {
        FOCUS_BUTTON_WIDTH
    }

    /// Fächer nebeneinander statt überlappend legen; passt er nicht ganz ins Bild, erscheint
    /// unter ihm ein [Slider3D], mit dem man ihn nach links und rechts schiebt (Start: Mitte).
    #[func]
    pub fn enable_scroll(&mut self) {
        self.enable_scroll_with(SCROLL_STEP, SCROLL_RADIUS);
    }

    /// Wie [method enable_scroll], mit eigenem Kartenabstand `step` (kleiner als eine
    /// Kartenbreite = überlappend) und Bogenradius `radius` (klein = stärker gebogen).
    #[func]
    pub fn enable_scroll_with(&mut self, step: f32, radius: f32) {
        let Some(mut zone) = self.zone.clone() else { return };
        for (prop, value) in [
            ("hand_radius", radius.to_variant()),
            ("hand_card_angle_degrees", (step / radius).to_degrees().to_variant()),
            ("hand_max_spread_degrees", 170.0f32.to_variant()),
        ] {
            zone.set(prop, &value);
        }
        zone.bind_mut().layout();
        let n = zone.bind().get_card_count().max(1) as f32;
        self.scroll_extra = ((n - 1.0) * step - SCROLL_SPAN).max(0.0);
        if self.scroll_extra <= 0.0 || self.slider.is_some() {
            return;
        }
        let mut slider = Slider3D::create(SLIDER_WIDTH, 0.5);
        slider.set_position(Vector3::new(0.0, SLIDER_Y, 0.05));
        let cb = Callable::from_object_method(&self.to_gd(), "on_scroll");
        slider.connect("value_changed", &cb);
        self.base_mut().add_child(&slider);
        self.slider = Some(slider);
    }

    /// Schieberegler bewegt (0 = linkes Ende sichtbar, 1 = rechtes Ende).
    #[func]
    fn on_scroll(&mut self, value: f32) {
        let Some(mut zone) = self.zone.clone() else { return };
        if self.coverflow {
            zone.set("coverflow_scroll", &((0.5 - value) * self.scroll_extra).to_variant());
            zone.bind_mut().layout();
            return;
        }
        let offset = (0.5 - value) * self.scroll_extra;
        let mut xf = zone.get_transform();
        xf.origin = FAN_CENTER + Vector3::new(offset, 0.0, 0.0);
        zone.set_transform(xf);
        zone.bind_mut().layout();
    }

    /// Fächer als Cover-Flow zum Durchsuchen vieler Karten (Bibliothek, Friedhof …), nach
    /// [method show_options] bzw. [method add_owned_cards] aufrufen: Karten in Größe `scale`
    /// im Abstand `step` überlappend nebeneinander, die mittlere vorn und gerade, die übrigen zur
    /// Seite gedreht und nach hinten versetzt, am Rand dicht gestapelt
    /// ([member Zone.coverflow]). Die Karte unter der Maus kommt groß und gerade nach vorn
    /// (Hover-Zoom). Bei vielen Karten erscheint darunter ein [Slider3D]; Schieben lässt die
    /// Karten durch die Mitte laufen (Start: Mitte).
    #[func]
    pub fn enable_coverflow(&mut self, step: f32, scale: f32) {
        let Some(mut zone) = self.zone.clone() else { return };
        self.coverflow = true;
        let mut xf = zone.get_transform();
        xf.origin = Vector3::new(0.0, COVERFLOW_CENTER_Y, 0.0);
        zone.set_transform(xf);
        for (prop, value) in [
            ("coverflow", true.to_variant()),
            ("coverflow_step", step.to_variant()),
            ("coverflow_scroll", 0.0f32.to_variant()),
            ("card_scale", scale.to_variant()),
            ("zoom_cards", true.to_variant()),
        ] {
            zone.set(prop, &value);
        }
        for card in zone.bind().get_cards().iter_shared() {
            let mut card = card;
            card.set("zoom_screen_height", &COVERFLOW_ZOOM_HEIGHT.to_variant());
        }
        zone.bind_mut().layout();
        let n = zone.bind().get_card_count().max(1) as f32;
        let width = (n - 1.0) * step;
        if width < COVERFLOW_SCROLL_MIN || self.slider.is_some() {
            return;
        }
        self.scroll_extra = width;
        let mut slider = Slider3D::create(SLIDER_WIDTH, 0.5);
        slider.set_position(Vector3::new(0.0, COVERFLOW_SLIDER_Y, 0.05));
        let cb = Callable::from_object_method(&self.to_gd(), "on_scroll");
        slider.connect("value_changed", &cb);
        self.base_mut().add_child(&slider);
        self.slider = Some(slider);
    }

    /// Knopf „Schlachtfeld ansehen“ oben rechts: blendet die Bühne samt Abdunkelung aus (der
    /// Tisch ist dann sichtbar und hoverbar), ein erneuter Druck („Zurück zur Auswahl“) holt sie
    /// zurück. Die Entscheidung bleibt dabei offen.
    #[func]
    pub fn enable_peek(&mut self) {
        if self.peek_button.is_some() {
            return;
        }
        let mut b = Button3D::create(ButtonStyle::Grey, tr!("Schlachtfeld ansehen", "View battlefield"), PEEK_WIDTH);
        b.set_position(Vector3::new(PEEK_POS.x, PEEK_POS.y, 0.05));
        let cb = Callable::from_object_method(&self.to_gd(), "on_peek");
        b.connect("pressed", &cb);
        self.base_mut().add_child(&b);
        self.peek_button = Some(b);
    }

    /// „Schlachtfeld ansehen“ gedrückt: Bühne aus- bzw. wieder einblenden.
    #[func]
    fn on_peek(&mut self) {
        if self.closing {
            return;
        }
        let peek = self.peek_button.clone();
        if self.peek_saved.is_empty() {
            for child in self.base().get_children().iter_shared() {
                let Ok(mut node) = child.try_cast::<Node3D>() else { continue };
                if peek.as_ref().is_some_and(|p| p.clone().upcast::<Node3D>() == node) {
                    continue;
                }
                let layer = node.clone().try_cast::<CollisionObject3D>().ok().map(|mut c| {
                    let l = c.get_collision_layer();
                    c.set_collision_layer(0);
                    l
                });
                self.peek_saved.push((node.clone(), node.is_visible(), layer));
                node.set_visible(false);
            }
        } else {
            for (mut node, visible, layer) in self.peek_saved.drain(..) {
                if !node.is_instance_valid() {
                    continue;
                }
                node.set_visible(visible);
                if let (Some(l), Ok(mut c)) = (layer, node.clone().try_cast::<CollisionObject3D>()) {
                    c.set_collision_layer(l);
                }
            }
        }
        let text = if self.peek_saved.is_empty() { tr!("Schlachtfeld ansehen", "View battlefield") } else { tr!("Zurück zur Auswahl", "Back to choice") };
        if let Some(mut b) = peek {
            b.bind_mut().set_label_text(GString::from(text));
        }
    }

    /// Weitere Ablage auf der Bühne (Zone der Art Hand in der Bühnenebene, Vorderseite zur
    /// Kamera): Mitte `center`, Größe `size`, Fächer mit Radius `radius` und Kartenabstand `step`
    /// (höchstens `max_spread` Grad); `drops` = Karten lassen sich hinein- und herausziehen.
    #[allow(clippy::too_many_arguments)]
    pub fn add_zone(&mut self, name: &str, center: Vector3, size: Vector3, radius: f32, step: f32, max_spread: f32, drops: bool) -> Gd<Zone> {
        let mut zone = Zone::new_alloc();
        zone.set_name(name);
        for (prop, value) in [
            ("kind", ZoneKind::Hand.to_variant()),
            ("size", size.to_variant()),
            ("accepts_drops", drops.to_variant()),
            ("cards_draggable", drops.to_variant()),
            ("face_up", true.to_variant()),
            ("hand_radius", radius.to_variant()),
            ("hand_card_angle_degrees", (step / radius).to_degrees().to_variant()),
            ("hand_max_spread_degrees", max_spread.to_variant()),
        ] {
            zone.set(prop, &value);
        }
        // Zone-Ebene: lokal y → zur Kamera, z → nach unten
        zone.set_transform(Transform3D::new(Basis::from_axis_angle(Vector3::RIGHT, std::f32::consts::FRAC_PI_2), center));
        self.base_mut().add_child(&zone);
        zone
    }

    /// Eigene Karten (Kopien) in die Ablage `zone` der Bühne legen; die Bühne gibt sie beim
    /// Schließen frei.
    pub fn add_owned_cards(&mut self, mut zone: Gd<Zone>, cards: Vec<Gd<Card3D>>) {
        let start = zone.get_global_transform();
        for mut card in cards {
            self.base_mut().add_child(&card);
            card.set_global_transform(start);
            card.bind_mut().set_bright(true);
            zone.bind_mut().add_card(card.clone(), -1, 0.0);
            self.owned.push(card);
        }
    }

    /// Dunkler Kasten mit hellem Rand auf der Bühne (z. B. Ablagefläche bei Hellsicht): Mitte
    /// und Größe in Bühnen-Koordinaten.
    pub fn add_frame(&mut self, center: Vector2, size: Vector2) {
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_shading_mode(ShadingMode::UNSHADED);
        mat.set_transparency(Transparency::ALPHA);
        mat.set_albedo(Color::from_rgba(0.03, 0.03, 0.05, 0.75));
        mat.set_render_priority(0);
        let mut quad = QuadMesh::new_gd();
        quad.set_size(size);
        quad.set_material(&mat);
        let mut frame = MeshInstance3D::new_alloc();
        frame.set_mesh(&quad);
        frame.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        frame.set_position(Vector3::new(center.x, center.y, -0.05));
        self.base_mut().add_child(&frame);
        // Rand als etwas größere, hellere Fläche dahinter
        let mut edge_mat = StandardMaterial3D::new_gd();
        edge_mat.set_shading_mode(ShadingMode::UNSHADED);
        edge_mat.set_transparency(Transparency::ALPHA);
        edge_mat.set_albedo(Color::from_rgba(0.55, 0.55, 0.6, 0.55));
        let mut edge_quad = QuadMesh::new_gd();
        edge_quad.set_size(size + Vector2::splat(0.012));
        edge_quad.set_material(&edge_mat);
        let mut edge = MeshInstance3D::new_alloc();
        edge.set_mesh(&edge_quad);
        edge.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        edge.set_position(Vector3::new(center.x, center.y, -0.052));
        self.base_mut().add_child(&edge);
    }

    /// Beschriftung auf der Bühne (z. B. „UNTEN“ / „OBEN“), linksbündig ab `pos`.
    pub fn add_caption(&mut self, text: &str, pos: Vector2, right_aligned: bool) {
        let mut l = Label3D::new_alloc();
        l.set_text(text);
        l.set_font_size(72);
        l.set_pixel_size(0.0013);
        l.set_outline_size(10);
        l.set_modulate(Color::from_rgb(0.95, 0.95, 0.95));
        l.set_outline_modulate(Color::from_rgba(0.02, 0.02, 0.03, 0.9));
        l.set_draw_flag(DrawFlags::SHADED, false);
        l.set_horizontal_alignment(if right_aligned { godot::global::HorizontalAlignment::RIGHT } else { godot::global::HorizontalAlignment::LEFT });
        l.set_position(Vector3::new(pos.x, pos.y, 0.05));
        self.base_mut().add_child(&l);
    }

    /// Der Hauptfächer der Bühne.
    pub fn fan_zone(&self) -> Option<Gd<Zone>> {
        self.zone.clone()
    }

    /// Breite eines Knopfes in dieser Bühne (Welteinheiten).
    #[func]
    pub fn button_width() -> f32 {
        BUTTON_WIDTH
    }

    /// Liegt `card` gerade in der Bühne?
    #[func]
    pub fn holds(&self, card: Gd<Card3D>) -> bool {
        // gelöschte Karten nie vergleichen (der Vergleich bräuchte ihre Instanz-ID)
        card.is_instance_valid() && self.borrowed.iter().any(|(c, _)| c.is_instance_valid() && *c == card)
    }

    /// Karten zurückgeben, Knöpfe ausblenden, abdunkeln aufheben und danach freigeben.
    #[func]
    pub fn close(&mut self) {
        if self.closing {
            return;
        }
        if let Some(mut zone) = self.zone.clone() {
            for (card, mut to) in self.borrowed.drain(..) {
                if card.is_instance_valid() && to.is_instance_valid() && card.bind().get_zone().as_ref() == Some(&zone) {
                    card.clone().bind_mut().set_bright(false);
                    zone.bind_mut().remove_card(card.clone());
                    to.bind_mut().add_card(card, -1, 0.0);
                }
            }
        }
        if !self.peek_saved.is_empty() {
            self.on_peek();
        }
        if let Some(mut b) = self.peek_button.take().filter(|b| b.is_instance_valid()) {
            b.bind_mut().dismiss();
        }
        if let Some(mut b) = self.blocker.take() {
            b.queue_free();
        }
        for mut b in self.buttons.drain(..) {
            if b.is_instance_valid() {
                b.bind_mut().dismiss();
            }
        }
        for card in self.owned.drain(..) {
            if card.is_instance_valid() {
                let mut card = card;
                card.queue_free();
            }
        }
        self.closing = true;
        self.t = 0.0;
    }
}

impl Showcase {
    fn build(&mut self) {
        // Abdunkelung hinter allem
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_shading_mode(ShadingMode::UNSHADED);
        mat.set_transparency(Transparency::ALPHA);
        mat.set_albedo(Color::from_rgba(0.02, 0.02, 0.04, 0.0));
        mat.set_render_priority(-1);
        let mut quad = QuadMesh::new_gd();
        quad.set_size(DIM_SIZE);
        quad.set_material(&mat);
        let mut dim = MeshInstance3D::new_alloc();
        dim.set_name("Dimmer");
        dim.set_mesh(&quad);
        dim.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        dim.set_position(Vector3::new(0.0, 0.0, -DIM_DEPTH));
        self.base_mut().add_child(&dim);
        self.dimmer = Some(mat);

        // Sperre auf dem Kartenlayer in der Ebene der Abdunkelung: der Mausstrahl trifft sie vor
        // den Tischkarten, aber hinter den Karten der Bühne
        let mut blocker = Area3D::new_alloc();
        blocker.set_name("Blocker");
        blocker.set_collision_layer(CARD_LAYER);
        blocker.set_collision_mask(0);
        blocker.set_monitoring(false);
        blocker.set_monitorable(false);
        let mut shape = BoxShape3D::new_gd();
        shape.set_size(Vector3::new(DIM_SIZE.x, DIM_SIZE.y, 0.01));
        let mut col = CollisionShape3D::new_alloc();
        col.set_shape(&shape);
        blocker.add_child(&col);
        blocker.set_position(Vector3::new(0.0, 0.0, -DIM_DEPTH));
        self.base_mut().add_child(&blocker);
        self.blocker = Some(blocker);

        // Kartenfächer: Zone (Hand) – Karten liegen in ihrer Ebene, Vorderseite zur Kamera
        let zone = self.add_zone("Fan", FAN_CENTER, Vector3::new(4.4, 0.1, 1.2), FAN_RADIUS, FAN_STEP, 40.0, false);
        self.zone = Some(zone);

        self.title = Some(self.label(0.92, 96, 0.0019, Color::from_rgb(0.98, 0.86, 0.55)));
        self.subtitle = Some(self.label(0.76, 64, 0.0013, Color::from_rgb(0.9, 0.86, 0.78)));
        self.note = Some(self.label(-0.93, 56, 0.0012, Color::from_rgb(0.9, 0.86, 0.78)));
    }

    fn label(&mut self, y: f32, font: i32, pixel: f32, color: Color) -> Gd<Label3D> {
        let mut l = Label3D::new_alloc();
        l.set_font_size(font);
        l.set_pixel_size(pixel);
        l.set_outline_size(font / 6);
        l.set_modulate(Color::from_rgba(color.r, color.g, color.b, 0.0));
        l.set_outline_modulate(Color::from_rgba(0.03, 0.02, 0.02, 0.85));
        l.set_draw_flag(DrawFlags::SHADED, false);
        l.set_position(Vector3::new(0.0, y, 0.05));
        l.set_visible(false);
        self.base_mut().add_child(&l);
        l
    }
}
