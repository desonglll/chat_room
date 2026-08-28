use crate::ai::{AiAssistant, AiTaskPlanDecision};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AgentIntent {
    Overview,
    Todos,
    Decisions,
    Search,
    General,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ContextScope {
    None,
    Recent,
    Full,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AgentPlan {
    pub intent: AgentIntent,
    pub context_scope: ContextScope,
    pub semantic_search: bool,
    pub research_questions: Vec<String>,
}

impl AgentPlan {
    pub fn intent_label(&self) -> &'static str {
        match self.intent {
            AgentIntent::Overview => "会话总结",
            AgentIntent::Todos => "提取待办",
            AgentIntent::Decisions => "提取结论",
            AgentIntent::Search => "查找事实",
            AgentIntent::General => "上下文问答",
        }
    }

    pub fn detail(&self) -> String {
        let context = match self.context_scope {
            ContextScope::None => "不读取聊天室",
            ContextScope::Recent => "读取近期上下文",
            ContextScope::Full => "读取全房间历史",
        };
        let retrieval = if self.semantic_search {
            "，并执行语义检索"
        } else {
            "，无需额外语义检索"
        };
        let research = match self.research_questions.len() {
            0 => String::new(),
            count => format!("；拆分 {count} 个研究子问题"),
        };
        format!(
            "任务：{}；{context}{retrieval}{research}",
            self.intent_label()
        )
    }
}

pub(super) async fn plan_request(
    assistant: &AiAssistant,
    question: &str,
    has_room: bool,
) -> anyhow::Result<AgentPlan> {
    if !has_room {
        return Ok(AgentPlan {
            intent: AgentIntent::General,
            context_scope: ContextScope::None,
            semantic_search: false,
            research_questions: Vec::new(),
        });
    }
    normalize_decision(question, assistant.plan_room_task(question).await?)
}

pub(super) fn fallback_plan(question: &str, has_room: bool) -> AgentPlan {
    if has_room && requests_room_wide_overview(question) {
        return room_wide_overview_plan();
    }
    AgentPlan {
        intent: AgentIntent::General,
        context_scope: if has_room {
            ContextScope::Recent
        } else {
            ContextScope::None
        },
        semantic_search: has_room,
        research_questions: Vec::new(),
    }
}

pub(super) fn catch_up_plan() -> AgentPlan {
    AgentPlan {
        intent: AgentIntent::Overview,
        context_scope: ContextScope::Recent,
        semantic_search: false,
        research_questions: Vec::new(),
    }
}

fn normalize_decision(question: &str, decision: AiTaskPlanDecision) -> anyhow::Result<AgentPlan> {
    if requests_room_wide_overview(question) {
        return Ok(room_wide_overview_plan());
    }
    let intent = match decision.intent.trim().to_ascii_lowercase().as_str() {
        "overview" => AgentIntent::Overview,
        "todos" => AgentIntent::Todos,
        "decisions" => AgentIntent::Decisions,
        "search" => AgentIntent::Search,
        "general" => AgentIntent::General,
        value => anyhow::bail!("planning agent returned unsupported intent: {value}"),
    };
    let context_scope = match decision.context_scope.trim().to_ascii_lowercase().as_str() {
        "recent" => ContextScope::Recent,
        "full" => ContextScope::Full,
        value => anyhow::bail!("planning agent returned unsupported context scope: {value}"),
    };
    let research_questions: Vec<String> = decision
        .research_questions
        .into_iter()
        .map(|question| question.trim().chars().take(240).collect::<String>())
        .filter(|question| !question.is_empty())
        .take(3)
        .collect();
    let semantic_search = decision.semantic_search || !research_questions.is_empty();
    Ok(AgentPlan {
        intent,
        context_scope,
        semantic_search,
        research_questions,
    })
}

fn room_wide_overview_plan() -> AgentPlan {
    AgentPlan {
        intent: AgentIntent::Overview,
        context_scope: ContextScope::Full,
        semantic_search: false,
        research_questions: Vec::new(),
    }
}

fn requests_room_wide_overview(question: &str) -> bool {
    let question = question.trim().to_lowercase();
    let explicitly_recent = ["最近", "近期", "刚才", "recent", "latest"]
        .iter()
        .any(|marker| question.contains(marker));
    if explicitly_recent {
        return false;
    }
    let room_wide = [
        "整个聊天室",
        "整个房间",
        "整个对话",
        "全部聊天",
        "全部消息",
        "所有聊天",
        "所有消息",
        "都聊了什么",
        "都聊了些什么",
        "full room",
        "entire room",
        "whole conversation",
        "entire conversation",
        "all messages",
        "full chat history",
    ]
    .iter()
    .any(|marker| question.contains(marker));
    let overview = [
        "聊了什么",
        "聊了些什么",
        "说了什么",
        "讨论了什么",
        "总结",
        "概括",
        "梳理",
        "summary",
        "summarize",
        "overview",
        "what was discussed",
    ]
    .iter()
    .any(|marker| question.contains(marker));
    room_wide && overview
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_a_model_generated_full_history_plan() {
        let plan = normalize_decision(
            "分析人员变化和技术决策",
            AiTaskPlanDecision {
                intent: "overview".into(),
                context_scope: "full".into(),
                semantic_search: false,
                research_questions: vec!["人员变化".into(), "技术决策".into()],
            },
        )
        .unwrap();
        assert_eq!(plan.intent, AgentIntent::Overview);
        assert_eq!(plan.context_scope, ContextScope::Full);
        assert!(plan.semantic_search);
        assert_eq!(plan.research_questions.len(), 2);
    }

    #[test]
    fn fallback_is_conservative_without_matching_local_keywords() {
        let plan = fallback_plan("普通问题", true);
        assert_eq!(plan.intent, AgentIntent::General);
        assert_eq!(plan.context_scope, ContextScope::Recent);
        assert!(plan.semantic_search);
    }

    #[test]
    fn research_questions_always_enable_semantic_search() {
        let plan = normalize_decision(
            "查找设计决策",
            AiTaskPlanDecision {
                intent: "general".into(),
                context_scope: "recent".into(),
                semantic_search: false,
                research_questions: vec!["设计决策".into()],
            },
        )
        .unwrap();

        assert!(plan.semantic_search);
    }

    #[test]
    fn explicit_room_wide_overview_overrides_a_recent_general_plan() {
        let plan = normalize_decision(
            "这个对话都聊了些什么",
            AiTaskPlanDecision {
                intent: "general".into(),
                context_scope: "recent".into(),
                semantic_search: true,
                research_questions: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(plan.intent, AgentIntent::Overview);
        assert_eq!(plan.context_scope, ContextScope::Full);
        assert!(!plan.semantic_search);
    }

    #[test]
    fn a_recent_overview_remains_bounded() {
        assert!(!requests_room_wide_overview("总结最近的聊天内容"));
    }
}
