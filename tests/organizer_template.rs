use suzuran_server::organizer::template::render_template;
use std::collections::HashMap;

fn tags(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn simple_field_substitution() {
    let t = tags(&[("albumartist", "Air"), ("title", "La Femme d'Argent")]);
    assert_eq!(render_template("{albumartist}/{title}", &t), "Air/La Femme d'Argent");
}

#[test]
fn missing_field_renders_empty() {
    assert_eq!(render_template("{title}", &tags(&[])), "");
}

#[test]
fn padded_track_number() {
    let t = tags(&[("tracknumber", "6")]);
    assert_eq!(render_template("{tracknumber:02}", &t), "06");
}

#[test]
fn padded_already_wide_number() {
    let t = tags(&[("tracknumber", "12")]);
    assert_eq!(render_template("{tracknumber:02}", &t), "12");
}

#[test]
fn fallback_used_when_field_absent() {
    assert_eq!(render_template("{albumartist|Various Artists}", &tags(&[])), "Various Artists");
}

#[test]
fn fallback_used_when_field_blank() {
    let t = tags(&[("albumartist", "")]);
    assert_eq!(render_template("{albumartist|Various Artists}", &t), "Various Artists");
}

#[test]
fn fallback_not_used_when_field_present() {
    let t = tags(&[("albumartist", "Air")]);
    assert_eq!(render_template("{albumartist|Various Artists}", &t), "Air");
}

#[test]
fn discfolder_multi_disc() {
    let t = tags(&[("totaldiscs", "2"), ("discnumber", "2")]);
    assert_eq!(render_template("{discfolder}", &t), "Disc 2/");
}

#[test]
fn discfolder_single_disc_suppressed() {
    let t = tags(&[("totaldiscs", "1"), ("discnumber", "1")]);
    assert_eq!(render_template("{discfolder}", &t), "");
}

#[test]
fn discfolder_absent_tags_suppressed() {
    assert_eq!(render_template("{discfolder}", &tags(&[])), "");
}

#[test]
fn full_template_multi_disc() {
    let t = tags(&[
        ("albumartist", "Pink Floyd"), ("date", "1979"), ("album", "The Wall"),
        ("totaldiscs", "2"), ("discnumber", "2"), ("tracknumber", "6"),
        ("title", "Comfortably Numb"),
    ]);
    assert_eq!(
        render_template(
            "{albumartist}/{date} - {album}/{discfolder}{tracknumber:02} - {title}",
            &t
        ),
        "Pink Floyd/1979 - The Wall/Disc 2/06 - Comfortably Numb"
    );
}

#[test]
fn full_template_single_disc() {
    let t = tags(&[
        ("albumartist", "Air"), ("date", "1998"), ("album", "Moon Safari"),
        ("totaldiscs", "1"), ("discnumber", "1"), ("tracknumber", "1"),
        ("title", "La Femme d'Argent"),
    ]);
    assert_eq!(
        render_template(
            "{albumartist}/{date} - {album}/{discfolder}{tracknumber:02} - {title}",
            &t
        ),
        "Air/1998 - Moon Safari/01 - La Femme d'Argent"
    );
}
