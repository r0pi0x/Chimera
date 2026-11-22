//! The voice and call-out files Combat Arms declares it plays, for the
//! asset pack (`chimera_pack`: `chimera_assets::resolve::declare`): what a
//! match can play whether or not a recorded run did. A client fetches them
//! with each match (`common`), so a rare line (a kill call-out, a radio
//! call, an announcer voice) is never fetched on demand mid-match.
//!
//! Read through the install's own records, by the game's own loaders:
//! `attributes/autocastsound.txt` (`kill_callout::CaTeamCallouts`: every
//! team call-out's file per sound template, men's and women's, the
//! infected's), `attributes/radiomsgsound.txt` (`chat::radio::RadioData`:
//! every entry's file per template), the announcer pack (`data/
//! announcer.toml`: its voices) and the announcer's weapon skins' voices
//! (`attributes/clientbutes.txt`: only the `*KillSound` keys of
//! `data::attributes::SKIN_VOICES`, not its other UI sounds).
//!
//! Known shortcuts: files only; the images the announcer draws aren't
//! listed (a base run reads them).

use lithtech_formats::attributes::Document;

use super::{
    chat::radio::{self, RadioData},
    data::attributes::{CLIENT_FILE, SKIN_VOICES},
    effects::kill_callout::{self, CaTeamCallouts},
    parts,
};

/// Every sound file Combat Arms declares, as the archive names them
/// (`/`, lower case), sorted and once each. `read` gives an install
/// file's bytes (None: not there).
pub fn voice_files(read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(callouts) = read(kill_callout::FILE).and_then(|b| CaTeamCallouts::parse(&String::from_utf8_lossy(&b)).ok()) {
        out.extend(callouts.0.iter().flat_map(|r| r.files.iter().flatten()).cloned());
    }
    if let Some(data) = read(radio::FILE).and_then(|b| RadioData::parse(&String::from_utf8_lossy(&b)).ok()) {
        out.extend(data.classes.iter().flat_map(|c| &c.entries).flat_map(|e| e.sounds.iter().flatten()).cloned());
    }
    if let Ok(pack) = chimera_core::announcer::AnnouncerPack::parse(&parts::ANNOUNCER) {
        out.extend(pack.files().1);
    }
    if let Some(doc) = read(CLIENT_FILE).and_then(|b| Document::parse(&b).ok()) {
        for section in doc.sections() {
            for (key, _) in SKIN_VOICES {
                if let Some(v) = section.string(key).filter(|v| !v.is_empty()) {
                    out.push(v.iter().map(|&b| b as char).collect());
                }
            }
        }
    }
    let mut out: Vec<String> = out
        .into_iter()
        .map(|f| f.trim().replace('\\', "/").to_ascii_lowercase())
        .filter(|f| f.ends_with(".wav"))
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The files of each source, normalized, once each; the skins' other
    /// keys (UI sounds) left out.
    #[test]
    fn the_declared_files_come_from_the_records() {
        let callouts = "[AutoCastTeams3DSound0]\nName =\"EnemyDown\"\n3DsoundRadius=1000.0\n\
                        Male_Snd0=\"CHARS_SND\\MALE\\VMA_EnemyDown_Ma.wav\"\n\
                        Female_Snd0=\"CHARS_SND\\Female\\VM_EnemyDown_Fa.wav\"\n";
        let radio = "[RadioClass]\nRadioClass0=\"Tactical\"\n[SoundTemplate]\nTemplate0=\"Male\"\n\
                     [Tactical0]\nMale_Snd0=\"UI_SND\\radiomsg\\Tactical\\Male\\VMA_gogogo_Ma.wav\"\n";
        let client = "[Psy]\nRevengeSound=\"UI_SND\\Revenge_PSY.wav\"\nMenuOpen=\"UI_SND\\menuopen.wav\"\n";
        let read = |p: &str| -> Option<Vec<u8>> {
            match p {
                kill_callout::FILE => Some(callouts.as_bytes().to_vec()),
                radio::FILE => Some(radio.as_bytes().to_vec()),
                CLIENT_FILE => Some(client.as_bytes().to_vec()),
                _ => None,
            }
        };
        let files = voice_files(&read);
        for f in [
            "chars_snd/male/vma_enemydown_ma.wav",
            "chars_snd/female/vm_enemydown_fa.wav",
            "ui_snd/radiomsg/tactical/male/vma_gogogo_ma.wav",
            "ui_snd/revenge_psy.wav",
            "ui_snd/doublekill.wav",
        ] {
            assert!(files.iter().any(|x| x == f), "{f} in {files:?}");
        }
        assert!(!files.iter().any(|f| f.contains("menuopen")), "{files:?}");
    }
}
