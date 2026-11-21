//! Dates de mc-core : des instants UTC, écrits en un seul endroit.
//!
//! Le cœur n'a besoin d'aucun fuseau horaire. Il lit l'heure, la date d'un
//! commit (secondes Unix) et celle d'un run CI (RFC 3339), compte des jours
//! entiers entre deux instants, et écrit deux formats, figés par les tests
//! ci-dessous :
//!
//! - RFC 3339 à la seconde, suffixe `Z` : `2023-11-14T22:13:20Z`. Ce texte est
//!   stocké tel quel (dates de création, journal) et la base trie dessus dans
//!   l'ordre lexicographique (`ORDER BY created_at`). Une fraction de seconde
//!   ou un `+00:00` ne changerait rien à l'instant, mais classerait mal les
//!   lignes écrites avant et après le changement ;
//! - l'horodatage compact des références de sauvegarde, `20231114T221320Z`.
//!
//! La bibliothèque est `time`, que Tauri compile déjà dans le binaire livré.
//! Elle remplace chrono, abandonnée par son mainteneur (RUSTSEC-2026-0543),
//! dont l'usage était dispersé dans cinq fichiers : d'où ce module.

use time::format_description::well_known::Rfc3339;
use time::format_description::BorrowedFormatItem;
use time::macros::format_description;
use time::UtcDateTime;

const HORODATAGE_SAUVEGARDE: &[BorrowedFormatItem<'_>] =
    format_description!("[year][month][day]T[hour][minute][second]Z");

/// L'instant présent en RFC 3339 à la seconde.
pub(crate) fn now_iso() -> String {
    rfc3339(UtcDateTime::now()).expect("l'année courante tient dans RFC 3339 (0000 à 9999)")
}

/// La date d'un commit git, donnée en secondes Unix, en RFC 3339 à la seconde.
///
/// Vide si l'instant sort des années 0000 à 9999, que RFC 3339 ne sait pas
/// écrire : seul un horodatage forgé y conduit.
pub(crate) fn iso_from_unix(seconds: i64) -> String {
    UtcDateTime::from_unix_timestamp(seconds)
        .ok()
        .and_then(rfc3339)
        .unwrap_or_default()
}

/// L'horodatage compact du nom d'une référence de sauvegarde.
pub(crate) fn backup_stamp(at: UtcDateTime) -> String {
    at.format(HORODATAGE_SAUVEGARDE)
        .expect("une date UTC fournit toutes les composantes du format")
}

/// Lit une date RFC 3339 telle que la renvoient GitHub (`Z`, sans fraction)
/// ou Azure DevOps (sept chiffres de fraction) ; un décalage horaire est
/// ramené à UTC. `None` si le texte n'est pas une date RFC 3339.
pub(crate) fn parse_rfc3339(text: &str) -> Option<UtcDateTime> {
    UtcDateTime::parse(text, &Rfc3339).ok()
}

/// Le nombre de jours entiers écoulés de `from` à `to`, tronqué vers zéro
/// (négatif si `from` est postérieur).
pub(crate) fn whole_days_between(from: UtcDateTime, to: UtcDateTime) -> i64 {
    (to - from).whole_days()
}

/// RFC 3339 à la seconde : la fraction est tronquée, jamais arrondie, comme
/// le faisait chrono avec `SecondsFormat::Secs`.
fn rfc3339(at: UtcDateTime) -> Option<String> {
    at.truncate_to_second().format(&Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instant(seconds: i64, nanoseconds: i128) -> UtcDateTime {
        UtcDateTime::from_unix_timestamp_nanos(i128::from(seconds) * 1_000_000_000 + nanoseconds)
            .unwrap()
    }

    #[test]
    fn ecrit_rfc3339_a_la_seconde_en_z() {
        assert_eq!(iso_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_from_unix(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(iso_from_unix(-1), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn tronque_la_fraction_sans_arrondir() {
        assert_eq!(
            rfc3339(instant(1_700_000_000, 999_999_999)).as_deref(),
            Some("2023-11-14T22:13:20Z")
        );
    }

    #[test]
    fn now_iso_garde_la_forme_stockee() {
        let texte = now_iso();
        assert_eq!(texte.len(), "2023-11-14T22:13:20Z".len(), "{texte}");
        assert!(texte.ends_with('Z') && !texte.contains('.'), "{texte}");
        assert_eq!(
            iso_from_unix(parse_rfc3339(&texte).unwrap().unix_timestamp()),
            texte
        );
    }

    #[test]
    fn rend_vide_hors_de_rfc3339() {
        // 10000-01-01T00:00:00Z : RFC 3339 n'écrit que quatre chiffres d'année.
        assert_eq!(iso_from_unix(253_402_300_800), "");
        assert_eq!(iso_from_unix(i64::MAX), "");
    }

    #[test]
    fn horodate_les_sauvegardes() {
        assert_eq!(backup_stamp(instant(1_700_000_000, 0)), "20231114T221320Z");
        assert_eq!(backup_stamp(instant(0, 999_999_999)), "19700101T000000Z");
    }

    #[test]
    fn lit_les_dates_des_plateformes_ci() {
        let attendu = instant(1_767_225_600, 0); // 2026-01-01T00:00:00Z
        assert_eq!(parse_rfc3339("2026-01-01T00:00:00Z"), Some(attendu));
        assert_eq!(
            parse_rfc3339("2026-01-01T00:00:00.1234567Z"),
            Some(instant(1_767_225_600, 123_456_700))
        );
        assert_eq!(parse_rfc3339("2026-01-01T02:00:00+02:00"), Some(attendu));
        assert_eq!(parse_rfc3339(""), None);
        assert_eq!(parse_rfc3339("2026-01-01"), None);
        assert_eq!(parse_rfc3339("pas une date"), None);
    }

    #[test]
    fn compte_les_jours_entiers() {
        let t0 = instant(1_700_000_000, 0);
        assert_eq!(
            whole_days_between(t0, instant(1_700_000_000 + 86_399, 0)),
            0
        );
        assert_eq!(
            whole_days_between(t0, instant(1_700_000_000 + 86_400, 0)),
            1
        );
        assert_eq!(
            whole_days_between(t0, instant(1_700_000_000 - 86_401, 0)),
            -1
        );
    }
}
