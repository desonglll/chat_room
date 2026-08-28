use std::collections::HashSet;

use super::models::AiCitationSource;

pub(super) fn select_image_sources(
    sources: &[AiCitationSource],
    question: &str,
    max_images: usize,
    max_total_images: usize,
    max_image_bytes: u64,
) -> Vec<AiCitationSource> {
    let mut candidates = sources
        .iter()
        .filter(|source| image_is_eligible(source, max_image_bytes))
        .cloned()
        .collect::<Vec<_>>();
    if is_broad_question(question) {
        candidates.truncate(max_total_images);
        return candidates;
    }
    if candidates.len() <= max_images {
        return candidates;
    }
    let question_terms = character_pairs(question);
    let mut ranked = candidates
        .into_iter()
        .enumerate()
        .map(|(ordinal, source)| {
            let attachment_name = source
                .attachment
                .as_ref()
                .map(|attachment| attachment.file_name.as_str())
                .unwrap_or_default();
            let candidate_terms = character_pairs(&format!(
                "{} {} {}",
                source.excerpt, source.sender, attachment_name
            ));
            let score = question_terms.intersection(&candidate_terms).count();
            (score, ordinal, source)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
    ranked.truncate(max_images);
    ranked.sort_by_key(|(_, ordinal, _)| *ordinal);
    ranked.into_iter().map(|(_, _, source)| source).collect()
}

fn image_is_eligible(source: &AiCitationSource, max_image_bytes: u64) -> bool {
    source.attachment.as_ref().is_some_and(|attachment| {
        !attachment.is_sensitive
            && attachment.mime_type.starts_with("image/")
            && attachment.size_bytes > 0
            && u64::try_from(attachment.size_bytes).is_ok_and(|size| size <= max_image_bytes)
    })
}

fn is_broad_question(question: &str) -> bool {
    let normalized = question.to_lowercase();
    [
        "整个",
        "全部",
        "所有",
        "都聊",
        "总结",
        "whole",
        "entire",
        "all messages",
        "overview",
        "summary",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn character_pairs(value: &str) -> HashSet<String> {
    let normalized = value
        .to_lowercase()
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect::<Vec<_>>();
    normalized
        .windows(2)
        .map(|pair| pair.iter().collect::<String>())
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use super::select_image_sources;
    use crate::ai_threads::{AiCitationAttachment, AiCitationSource};

    fn source(index: i64, excerpt: &str) -> AiCitationSource {
        AiCitationSource {
            label: format!("A{}", index + 1),
            room_id: Uuid::nil(),
            message_id: Uuid::new_v4(),
            sender: "Ada".into(),
            sent_at: Utc::now() + Duration::seconds(index),
            excerpt: excerpt.into(),
            score: None,
            score_kind: "attachment".into(),
            attachment: Some(AiCitationAttachment {
                id: Uuid::new_v4(),
                file_name: format!("image-{index}.png"),
                mime_type: "image/png".into(),
                size_bytes: 1_024,
                download_url: "/private".into(),
                is_sensitive: false,
            }),
        }
    }

    #[test]
    fn targeted_visual_selection_prefers_related_message_context() {
        let sources = vec![
            source(0, "午餐照片"),
            source(1, "发布计划和截止日期"),
            source(2, "周末天气"),
        ];
        let selected = select_image_sources(&sources, "发布计划是什么？", 1, 20, 8 * 1024 * 1024);
        assert_eq!(selected[0].label, "A2");
    }

    #[test]
    fn broad_visual_selection_keeps_the_full_room_chronology_for_batched_processing() {
        let sources = (0..9)
            .map(|index| source(index, "聊天截图"))
            .collect::<Vec<_>>();
        let selected = select_image_sources(&sources, "总结整个聊天室", 3, 20, 8 * 1024 * 1024);
        assert_eq!(
            selected
                .iter()
                .map(|source| source.label.as_str())
                .collect::<Vec<_>>(),
            ["A1", "A2", "A3", "A4", "A5", "A6", "A7", "A8", "A9"]
        );
    }
}
