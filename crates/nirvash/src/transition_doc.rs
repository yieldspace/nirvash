use std::{
    any::{Any, TypeId},
    collections::{BTreeMap, BTreeSet},
};

use serde::{Deserialize, Serialize};

use crate::{
    BoolExpr, BuiltinPredicateOp, ComparisonOp, DocGraphActionPresentation, DocGraphState,
    ErasedGuardValueExprAst, ErasedStateExprAst, ErasedStepValueExprAst, GuardAst, GuardExpr,
    ModelBackend, QuantifierKind, ReachableGraphSnapshot, SpecVizKind, SpecVizRegistrationSet,
    SpecVizSubsystem, StepExpr, TransitionProgram, TransitionRule, TrustTier, UpdateAst, UpdateOp,
    UpdateValueExprAst,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum TransitionDocReachabilityMode {
    Always,
    Never,
    #[default]
    AutoIfFinite,
}

impl TransitionDocReachabilityMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::Never => "never",
            Self::AutoIfFinite => "auto_if_finite",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TransitionDocMetadata {
    pub spec_id: String,
    pub kind: Option<SpecVizKind>,
    pub state_ty: String,
    pub action_ty: String,
    pub model_cases: Option<String>,
    pub subsystems: Vec<SpecVizSubsystem>,
    pub registrations: SpecVizRegistrationSet,
    pub reachability_mode: TransitionDocReachabilityMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TransitionDocBundle {
    pub spec_name: String,
    pub metadata: TransitionDocMetadata,
    pub structure_cases: Vec<TransitionDocStructureCase>,
    pub reachability_cases: Vec<TransitionDocReachabilityCase>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TransitionDocStructureCase {
    pub label: String,
    pub program_name: String,
    pub action_patterns: Vec<String>,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub rules: Vec<TransitionDocRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TransitionDocRule {
    pub name: String,
    pub action_patterns: Vec<String>,
    pub guard: String,
    pub update: String,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub effects: Vec<String>,
    pub deterministic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionDocReachabilityCase {
    pub label: String,
    pub backend: ModelBackend,
    pub trust_tier: TrustTier,
    pub surface: Option<String>,
    pub projection: Option<String>,
    pub states: Vec<TransitionDocStateNode>,
    pub edges: Vec<Vec<TransitionDocStateEdge>>,
    pub initial_indices: Vec<usize>,
    pub deadlocks: Vec<usize>,
    pub truncated: bool,
    pub stutter_omitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TransitionDocStateNode {
    pub summary: String,
    pub full: String,
    pub relation_fields: Vec<crate::RelationFieldSummary>,
    pub relation_schema: Vec<crate::RelationFieldSchema>,
}

impl From<DocGraphState> for TransitionDocStateNode {
    fn from(value: DocGraphState) -> Self {
        Self {
            summary: value.summary,
            full: value.full,
            relation_fields: value.relation_fields,
            relation_schema: value.relation_schema,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TransitionDocStateEdge {
    pub label: String,
    pub compact_label: Option<String>,
    pub target: usize,
}

pub trait TransitionDocProvider {
    fn spec_name(&self) -> &'static str;
    fn bundle(&self) -> TransitionDocBundle;
}

#[derive(Clone, Copy)]
pub struct RegisteredTransitionDocProvider {
    pub spec_name: &'static str,
    pub build: fn() -> Box<dyn TransitionDocProvider>,
}

#[derive(Clone, Copy)]
pub struct RegisteredTransitionDocSpecConfig {
    pub spec_type_id: fn() -> TypeId,
    pub reachability_mode: TransitionDocReachabilityMode,
    pub build_cases: Option<fn() -> Box<dyn Any>>,
}

#[derive(Clone, Copy)]
pub struct RegisteredTransitionDocCase {
    pub spec_type_id: fn() -> TypeId,
    pub name: &'static str,
    pub build: fn() -> Box<dyn Any>,
}

inventory::collect!(RegisteredTransitionDocProvider);
inventory::collect!(RegisteredTransitionDocSpecConfig);
inventory::collect!(RegisteredTransitionDocCase);

pub fn collect_transition_doc_provider_registrations() -> Vec<RegisteredTransitionDocProvider> {
    let mut registrations = inventory::iter::<RegisteredTransitionDocProvider>
        .into_iter()
        .copied()
        .collect::<Vec<_>>();
    registrations.sort_by_key(|entry| entry.spec_name);
    registrations
}

pub fn collect_transition_doc_bundles() -> Vec<TransitionDocBundle> {
    let mut bundles_by_name = BTreeMap::<String, TransitionDocBundle>::new();
    for registration in collect_transition_doc_provider_registrations() {
        let bundle = (registration.build)().bundle();
        assert!(
            bundles_by_name
                .insert(bundle.spec_name.clone(), bundle)
                .is_none(),
            "duplicate transition doc provider registration for spec `{}`",
            registration.spec_name
        );
    }
    bundles_by_name.into_values().collect()
}

pub fn transition_doc_reachability_mode_for<Spec>() -> TransitionDocReachabilityMode
where
    Spec: 'static,
{
    let spec_type_id = TypeId::of::<Spec>();
    let matched = inventory::iter::<RegisteredTransitionDocSpecConfig>
        .into_iter()
        .filter(|entry| (entry.spec_type_id)() == spec_type_id)
        .collect::<Vec<_>>();
    assert!(
        matched.len() <= 1,
        "multiple #[doc_spec] registrations for spec `{}`",
        std::any::type_name::<Spec>()
    );
    matched
        .into_iter()
        .next()
        .map(|entry| entry.reachability_mode)
        .unwrap_or_default()
}

pub fn transition_doc_spec_cases_for<Spec>() -> Option<Vec<Spec>>
where
    Spec: 'static,
{
    let spec_type_id = TypeId::of::<Spec>();
    let matched = inventory::iter::<RegisteredTransitionDocSpecConfig>
        .into_iter()
        .filter(|entry| (entry.spec_type_id)() == spec_type_id)
        .collect::<Vec<_>>();
    assert!(
        matched.len() <= 1,
        "multiple #[doc_spec] registrations for spec `{}`",
        std::any::type_name::<Spec>()
    );
    let Some(entry) = matched.into_iter().next() else {
        return None;
    };
    entry.build_cases.map(|build| {
        *build()
            .downcast::<Vec<Spec>>()
            .unwrap_or_else(|_| {
                panic!(
                    "#[doc_spec] cases(...) for spec `{}` must return Vec<{}>",
                    std::any::type_name::<Spec>(),
                    std::any::type_name::<Spec>()
                )
            })
    })
}

pub fn build_transition_doc_structure_case<S, A>(
    label: impl Into<String>,
    program: &TransitionProgram<S, A>,
) -> TransitionDocStructureCase
where
    S: 'static,
    A: 'static,
{
    let mut action_patterns = BTreeSet::new();
    let mut reads = BTreeSet::new();
    let mut writes = BTreeSet::new();
    let rules = program
        .rules()
        .iter()
        .map(|rule| {
            let rule_action_patterns = collect_rule_action_patterns(rule);
            let rule_reads = collect_rule_reads(rule);
            let rule_writes = collect_rule_writes(rule);
            let rule_effects = collect_rule_effects(rule);
            for pattern in &rule_action_patterns {
                action_patterns.insert(pattern.clone());
            }
            for path in &rule_reads {
                reads.insert(path.clone());
            }
            for path in &rule_writes {
                writes.insert(path.clone());
            }
            TransitionDocRule {
                name: rule.name().to_owned(),
                action_patterns: rule_action_patterns,
                guard: format_rule_guard(rule),
                update: format_rule_update(rule),
                reads: rule_reads,
                writes: rule_writes,
                effects: rule_effects,
                deterministic: !matches!(rule.update_ast(), Some(UpdateAst::Choice(_))),
            }
        })
        .collect();

    TransitionDocStructureCase {
        label: label.into(),
        program_name: program.name().to_owned(),
        action_patterns: action_patterns.into_iter().collect(),
        reads: reads.into_iter().collect(),
        writes: writes.into_iter().collect(),
        rules,
    }
}

pub fn build_transition_doc_reachability_case<S, A>(
    label: impl Into<String>,
    surface: Option<String>,
    projection: Option<String>,
    backend: ModelBackend,
    snapshot: ReachableGraphSnapshot<S, A>,
    summarize_state: impl Fn(&S) -> DocGraphState,
    summarize_action: impl Fn(&A) -> DocGraphActionPresentation,
) -> TransitionDocReachabilityCase {
    TransitionDocReachabilityCase {
        label: label.into(),
        backend,
        trust_tier: snapshot.trust_tier,
        surface,
        projection,
        states: snapshot
            .states
            .iter()
            .map(|state| summarize_state(state).into())
            .collect(),
        edges: snapshot
            .edges
            .iter()
            .map(|outgoing| {
                outgoing
                    .iter()
                    .map(|edge| {
                        let presentation = summarize_action(&edge.action);
                        TransitionDocStateEdge {
                            label: presentation.label,
                            compact_label: presentation.compact_label,
                            target: edge.target,
                        }
                    })
                    .collect()
            })
            .collect(),
        initial_indices: snapshot.initial_indices,
        deadlocks: snapshot.deadlocks,
        truncated: snapshot.truncated,
        stutter_omitted: snapshot.stutter_omitted,
    }
}

pub fn has_registered_finite_domain<T>() -> bool
where
    T: 'static,
{
    !crate::lookup_finite_domain_seed_values::<T>().is_empty()
}

fn collect_rule_action_patterns<S, A>(rule: &TransitionRule<S, A>) -> Vec<String>
where
    S: 'static,
    A: 'static,
{
    let mut patterns = BTreeSet::new();
    if let Some(ast) = rule.guard_ast() {
        collect_guard_action_patterns(ast, &mut patterns);
    }
    patterns.into_iter().collect()
}

fn collect_guard_action_patterns<S, A>(ast: &GuardAst<S, A>, patterns: &mut BTreeSet<String>)
where
    S: 'static,
    A: 'static,
{
    match ast {
        GuardAst::Match(matcher) => {
            if matcher.value().starts_with("action") {
                patterns.insert(normalize_tokens(matcher.pattern()));
            }
        }
        GuardAst::Not(inner) => {
            if let Some(ast) = inner.ast() {
                collect_guard_action_patterns(ast, patterns);
            }
        }
        GuardAst::And(parts) | GuardAst::Or(parts) => {
            for part in parts {
                if let Some(ast) = part.ast() {
                    collect_guard_action_patterns(ast, patterns);
                }
            }
        }
        GuardAst::Literal(_)
        | GuardAst::FieldRead(_)
        | GuardAst::PureCall(_)
        | GuardAst::Eq(_)
        | GuardAst::Ne(_)
        | GuardAst::Lt(_)
        | GuardAst::Le(_)
        | GuardAst::Gt(_)
        | GuardAst::Ge(_)
        | GuardAst::Contains(_)
        | GuardAst::SubsetOf(_)
        | GuardAst::ForAll(_)
        | GuardAst::Exists(_) => {}
    }
}

fn collect_rule_reads<S, A>(rule: &TransitionRule<S, A>) -> Vec<String>
where
    S: 'static,
    A: 'static,
{
    rule.symbolic_full_read_paths()
        .into_iter()
        .map(normalize_tokens)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn collect_rule_writes<S, A>(rule: &TransitionRule<S, A>) -> Vec<String>
where
    S: 'static,
    A: 'static,
{
    let mut writes = BTreeSet::new();
    if let Some(ast) = rule.update_ast() {
        collect_update_writes(ast, &mut writes);
    }
    writes.into_iter().collect()
}

fn collect_update_writes<S: 'static, A: 'static>(
    ast: &UpdateAst<S, A>,
    writes: &mut BTreeSet<String>,
) {
    match ast {
        UpdateAst::Sequence(ops) => {
            for op in ops {
                match op {
                    UpdateOp::Assign { target, .. }
                    | UpdateOp::SetInsert { target, .. }
                    | UpdateOp::SetRemove { target, .. } => {
                        writes.insert(normalize_tokens(target));
                    }
                    UpdateOp::Effect { .. } => {}
                }
            }
        }
        UpdateAst::Choice(choice) => {
            for path in choice.write_paths() {
                writes.insert(normalize_tokens(path));
            }
        }
    }
}

fn collect_rule_effects<S, A>(rule: &TransitionRule<S, A>) -> Vec<String>
where
    S: 'static,
    A: 'static,
{
    rule.effect_names()
        .into_iter()
        .map(normalize_tokens)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn format_rule_guard<S, A>(rule: &TransitionRule<S, A>) -> String
where
    S: 'static,
    A: 'static,
{
    rule.guard_ast()
        .map(format_guard_ast)
        .unwrap_or_else(|| "true".to_owned())
}

fn format_rule_update<S, A>(rule: &TransitionRule<S, A>) -> String
where
    S: 'static,
    A: 'static,
{
    rule.update_ast()
        .map(format_update_ast)
        .unwrap_or_else(|| "noop".to_owned())
}

#[allow(dead_code)]
fn format_bool_expr<S>(expr: &BoolExpr<S>) -> String
where
    S: 'static,
{
    expr.ast()
        .map(format_bool_ast)
        .unwrap_or_else(|| normalize_tokens(expr.name()))
}

#[allow(dead_code)]
fn format_bool_ast<S>(ast: &crate::BoolExprAst<S>) -> String
where
    S: 'static,
{
    match ast {
        crate::BoolExprAst::Literal(value) => value.to_string(),
        crate::BoolExprAst::FieldRead(field) => normalize_tokens(field.label()),
        crate::BoolExprAst::PureCall(field) => normalize_tokens(field.label()),
        crate::BoolExprAst::Eq(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        crate::BoolExprAst::Ne(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        crate::BoolExprAst::Lt(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        crate::BoolExprAst::Le(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        crate::BoolExprAst::Gt(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        crate::BoolExprAst::Ge(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        crate::BoolExprAst::Contains(predicate) => format_comparison(
            predicate.lhs(),
            builtin_predicate_op_text(predicate.op()),
            predicate.rhs(),
        ),
        crate::BoolExprAst::SubsetOf(predicate) => format_comparison(
            predicate.lhs(),
            builtin_predicate_op_text(predicate.op()),
            predicate.rhs(),
        ),
        crate::BoolExprAst::Match(matcher) => {
            format_match(matcher.value(), matcher.pattern())
        }
        crate::BoolExprAst::ForAll(quantifier) | crate::BoolExprAst::Exists(quantifier) => {
            format_quantifier(quantifier.kind(), quantifier.domain(), quantifier.body())
        }
        crate::BoolExprAst::Not(inner) => format!("not ({})", format_bool_expr(inner)),
        crate::BoolExprAst::And(parts) => format_joined_bool("and", parts, format_bool_expr),
        crate::BoolExprAst::Or(parts) => format_joined_bool("or", parts, format_bool_expr),
    }
}

#[allow(dead_code)]
fn format_step_expr<S, A>(expr: &StepExpr<S, A>) -> String
where
    S: 'static,
    A: 'static,
{
    expr.ast()
        .map(format_step_ast)
        .unwrap_or_else(|| normalize_tokens(expr.name()))
}

#[allow(dead_code)]
fn format_step_ast<S, A>(ast: &crate::StepExprAst<S, A>) -> String
where
    S: 'static,
    A: 'static,
{
    match ast {
        crate::StepExprAst::Literal(value) => value.to_string(),
        crate::StepExprAst::FieldRead(field) => normalize_tokens(field.label()),
        crate::StepExprAst::PureCall(field) => normalize_tokens(field.label()),
        crate::StepExprAst::Eq(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        crate::StepExprAst::Ne(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        crate::StepExprAst::Lt(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        crate::StepExprAst::Le(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        crate::StepExprAst::Gt(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        crate::StepExprAst::Ge(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        crate::StepExprAst::Contains(predicate) => format_comparison(
            predicate.lhs(),
            builtin_predicate_op_text(predicate.op()),
            predicate.rhs(),
        ),
        crate::StepExprAst::SubsetOf(predicate) => format_comparison(
            predicate.lhs(),
            builtin_predicate_op_text(predicate.op()),
            predicate.rhs(),
        ),
        crate::StepExprAst::Match(matcher) => {
            format_match(matcher.value(), matcher.pattern())
        }
        crate::StepExprAst::ForAll(quantifier) | crate::StepExprAst::Exists(quantifier) => {
            format_quantifier(quantifier.kind(), quantifier.domain(), quantifier.body())
        }
        crate::StepExprAst::Not(inner) => format!("not ({})", format_step_expr(inner)),
        crate::StepExprAst::And(parts) => format_joined_bool("and", parts, format_step_expr),
        crate::StepExprAst::Or(parts) => format_joined_bool("or", parts, format_step_expr),
    }
}

fn format_guard_expr<S, A>(expr: &GuardExpr<S, A>) -> String
where
    S: 'static,
    A: 'static,
{
    expr.ast()
        .map(format_guard_ast)
        .unwrap_or_else(|| normalize_tokens(expr.name()))
}

fn format_guard_ast<S, A>(ast: &GuardAst<S, A>) -> String
where
    S: 'static,
    A: 'static,
{
    match ast {
        GuardAst::Literal(value) => value.to_string(),
        GuardAst::FieldRead(field) => normalize_tokens(field.label()),
        GuardAst::PureCall(field) => normalize_tokens(field.label()),
        GuardAst::Eq(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        GuardAst::Ne(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        GuardAst::Lt(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        GuardAst::Le(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        GuardAst::Gt(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        GuardAst::Ge(compare) => {
            format_comparison(compare.lhs(), comparison_op_text(compare.op()), compare.rhs())
        }
        GuardAst::Contains(predicate) => format_comparison(
            predicate.lhs(),
            builtin_predicate_op_text(predicate.op()),
            predicate.rhs(),
        ),
        GuardAst::SubsetOf(predicate) => format_comparison(
            predicate.lhs(),
            builtin_predicate_op_text(predicate.op()),
            predicate.rhs(),
        ),
        GuardAst::Match(matcher) => format_match(matcher.value(), matcher.pattern()),
        GuardAst::ForAll(quantifier) | GuardAst::Exists(quantifier) => {
            format_quantifier(quantifier.kind(), quantifier.domain(), quantifier.body())
        }
        GuardAst::Not(inner) => format!("not ({})", format_guard_expr(inner)),
        GuardAst::And(parts) => format_joined_bool("and", parts, format_guard_expr),
        GuardAst::Or(parts) => format_joined_bool("or", parts, format_guard_expr),
    }
}

fn format_update_ast<S, A>(ast: &UpdateAst<S, A>) -> String
where
    S: 'static,
    A: 'static,
{
    match ast {
        UpdateAst::Sequence(ops) => ops
            .iter()
            .map(|op| match op {
                UpdateOp::Assign { target, value_ast, .. } => {
                    format!("set {} <= {}", normalize_tokens(target), format_update_value_expr(value_ast))
                }
                UpdateOp::SetInsert { target, item_ast, .. } => format!(
                    "insert {} <= {}",
                    normalize_tokens(target),
                    format_update_value_expr(item_ast)
                ),
                UpdateOp::SetRemove { target, item_ast, .. } => format!(
                    "remove {} <= {}",
                    normalize_tokens(target),
                    format_update_value_expr(item_ast)
                ),
                UpdateOp::Effect { name, .. } => format!("effect {}", normalize_tokens(name)),
            })
            .collect::<Vec<_>>()
            .join(";\n"),
        UpdateAst::Choice(choice) => format!(
            "choose {} where {} -> writes {}",
            normalize_tokens(choice.domain()),
            normalize_tokens(choice.body()),
            choice
                .write_paths()
                .iter()
                .map(|path| normalize_tokens(path))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn format_update_value_expr<S, A>(expr: &UpdateValueExprAst<S, A>) -> String
where
    S: 'static,
    A: 'static,
{
    match expr {
        UpdateValueExprAst::Opaque { repr } | UpdateValueExprAst::Literal { repr } => {
            normalize_tokens(repr)
        }
        UpdateValueExprAst::FieldRead { path } => normalize_tokens(path),
        UpdateValueExprAst::PureCall { name, .. } => normalize_tokens(name),
        UpdateValueExprAst::Add { lhs, rhs } => {
            format_binary_value_expr(lhs, "+", rhs, |expr| format_update_value_expr(expr))
        }
        UpdateValueExprAst::Sub { lhs, rhs } => {
            format_binary_value_expr(lhs, "-", rhs, |expr| format_update_value_expr(expr))
        }
        UpdateValueExprAst::Mul { lhs, rhs } => {
            format_binary_value_expr(lhs, "*", rhs, |expr| format_update_value_expr(expr))
        }
        UpdateValueExprAst::Neg { expr } => format!("-{}", format_update_value_expr(expr)),
        UpdateValueExprAst::Union { lhs, rhs } => {
            format_binary_value_expr(lhs, "union", rhs, |expr| format_update_value_expr(expr))
        }
        UpdateValueExprAst::Intersection { lhs, rhs } => {
            format_binary_value_expr(lhs, "intersection", rhs, |expr| format_update_value_expr(expr))
        }
        UpdateValueExprAst::Difference { lhs, rhs } => {
            format_binary_value_expr(lhs, "difference", rhs, |expr| format_update_value_expr(expr))
        }
        UpdateValueExprAst::SequenceUpdate { base, index, value } => format!(
            "{}[{} := {}]",
            format_update_value_expr(base),
            format_update_value_expr(index),
            format_update_value_expr(value)
        ),
        UpdateValueExprAst::FunctionUpdate { base, key, value } => format!(
            "{}[{} := {}]",
            format_update_value_expr(base),
            format_update_value_expr(key),
            format_update_value_expr(value)
        ),
        UpdateValueExprAst::RecordUpdate { base, field, value } => format!(
            "{}{{{} := {}}}",
            format_update_value_expr(base),
            normalize_tokens(field),
            format_update_value_expr(value)
        ),
        UpdateValueExprAst::Comprehension { domain, body, .. } => format!(
            "{{ {} | {} }}",
            normalize_tokens(domain),
            normalize_tokens(body)
        ),
        UpdateValueExprAst::IfElse {
            condition,
            then_branch,
            else_branch,
        } => format!(
            "if {} then {} else {}",
            format_guard_expr(condition),
            format_update_value_expr(then_branch),
            format_update_value_expr(else_branch)
        ),
        UpdateValueExprAst::_Phantom(_) => "<phantom>".to_owned(),
    }
}

#[allow(dead_code)]
fn format_guard_value_expr<S, A>(expr: &ErasedGuardValueExprAst<S, A>) -> String
where
    S: 'static,
    A: 'static,
{
    match expr {
        ErasedGuardValueExprAst::Opaque { repr } | ErasedGuardValueExprAst::Literal { repr } => {
            normalize_tokens(repr)
        }
        ErasedGuardValueExprAst::FieldRead { path } => normalize_tokens(path),
        ErasedGuardValueExprAst::PureCall { name, .. } => normalize_tokens(name),
        ErasedGuardValueExprAst::Add { lhs, rhs } => {
            format_binary_value_expr(lhs, "+", rhs, |expr| format_guard_value_expr(expr))
        }
        ErasedGuardValueExprAst::Sub { lhs, rhs } => {
            format_binary_value_expr(lhs, "-", rhs, |expr| format_guard_value_expr(expr))
        }
        ErasedGuardValueExprAst::Mul { lhs, rhs } => {
            format_binary_value_expr(lhs, "*", rhs, |expr| format_guard_value_expr(expr))
        }
        ErasedGuardValueExprAst::Neg { expr } => format!("-{}", format_guard_value_expr(expr)),
        ErasedGuardValueExprAst::Union { lhs, rhs } => {
            format_binary_value_expr(lhs, "union", rhs, |expr| format_guard_value_expr(expr))
        }
        ErasedGuardValueExprAst::Intersection { lhs, rhs } => {
            format_binary_value_expr(lhs, "intersection", rhs, |expr| format_guard_value_expr(expr))
        }
        ErasedGuardValueExprAst::Difference { lhs, rhs } => {
            format_binary_value_expr(lhs, "difference", rhs, |expr| format_guard_value_expr(expr))
        }
        ErasedGuardValueExprAst::Comprehension { domain, body, .. } => format!(
            "{{ {} | {} }}",
            normalize_tokens(domain),
            normalize_tokens(body)
        ),
        ErasedGuardValueExprAst::IfElse {
            condition,
            then_branch,
            else_branch,
        } => format!(
            "if {} then {} else {}",
            format_guard_expr(condition),
            format_guard_value_expr(then_branch),
            format_guard_value_expr(else_branch)
        ),
    }
}

#[allow(dead_code)]
fn format_state_value_expr<S>(expr: &ErasedStateExprAst<S>) -> String
where
    S: 'static,
{
    match expr {
        ErasedStateExprAst::Opaque { repr } | ErasedStateExprAst::Literal { repr } => {
            normalize_tokens(repr)
        }
        ErasedStateExprAst::FieldRead { path } => normalize_tokens(path),
        ErasedStateExprAst::PureCall { name, .. } => normalize_tokens(name),
        ErasedStateExprAst::Add { lhs, rhs } => {
            format_binary_value_expr(lhs, "+", rhs, |expr| format_state_value_expr(expr))
        }
        ErasedStateExprAst::Sub { lhs, rhs } => {
            format_binary_value_expr(lhs, "-", rhs, |expr| format_state_value_expr(expr))
        }
        ErasedStateExprAst::Mul { lhs, rhs } => {
            format_binary_value_expr(lhs, "*", rhs, |expr| format_state_value_expr(expr))
        }
        ErasedStateExprAst::Neg { expr } => format!("-{}", format_state_value_expr(expr)),
        ErasedStateExprAst::Union { lhs, rhs } => {
            format_binary_value_expr(lhs, "union", rhs, |expr| format_state_value_expr(expr))
        }
        ErasedStateExprAst::Intersection { lhs, rhs } => {
            format_binary_value_expr(lhs, "intersection", rhs, |expr| format_state_value_expr(expr))
        }
        ErasedStateExprAst::Difference { lhs, rhs } => {
            format_binary_value_expr(lhs, "difference", rhs, |expr| format_state_value_expr(expr))
        }
        ErasedStateExprAst::Comprehension { domain, body, .. } => format!(
            "{{ {} | {} }}",
            normalize_tokens(domain),
            normalize_tokens(body)
        ),
        ErasedStateExprAst::IfElse {
            condition,
            then_branch,
            else_branch,
        } => format!(
            "if {} then {} else {}",
            format_bool_expr(condition),
            format_state_value_expr(then_branch),
            format_state_value_expr(else_branch)
        ),
    }
}

#[allow(dead_code)]
fn format_step_value_expr<S, A>(expr: &ErasedStepValueExprAst<S, A>) -> String
where
    S: 'static,
    A: 'static,
{
    match expr {
        ErasedStepValueExprAst::Opaque { repr } | ErasedStepValueExprAst::Literal { repr } => {
            normalize_tokens(repr)
        }
        ErasedStepValueExprAst::FieldRead { path } => normalize_tokens(path),
        ErasedStepValueExprAst::PureCall { name, .. } => normalize_tokens(name),
        ErasedStepValueExprAst::Add { lhs, rhs } => {
            format_binary_value_expr(lhs, "+", rhs, |expr| format_step_value_expr(expr))
        }
        ErasedStepValueExprAst::Sub { lhs, rhs } => {
            format_binary_value_expr(lhs, "-", rhs, |expr| format_step_value_expr(expr))
        }
        ErasedStepValueExprAst::Mul { lhs, rhs } => {
            format_binary_value_expr(lhs, "*", rhs, |expr| format_step_value_expr(expr))
        }
        ErasedStepValueExprAst::Neg { expr } => format!("-{}", format_step_value_expr(expr)),
        ErasedStepValueExprAst::Union { lhs, rhs } => {
            format_binary_value_expr(lhs, "union", rhs, |expr| format_step_value_expr(expr))
        }
        ErasedStepValueExprAst::Intersection { lhs, rhs } => {
            format_binary_value_expr(lhs, "intersection", rhs, |expr| format_step_value_expr(expr))
        }
        ErasedStepValueExprAst::Difference { lhs, rhs } => {
            format_binary_value_expr(lhs, "difference", rhs, |expr| format_step_value_expr(expr))
        }
        ErasedStepValueExprAst::Comprehension { domain, body, .. } => format!(
            "{{ {} | {} }}",
            normalize_tokens(domain),
            normalize_tokens(body)
        ),
        ErasedStepValueExprAst::IfElse {
            condition,
            then_branch,
            else_branch,
        } => format!(
            "if {} then {} else {}",
            format_step_expr(condition),
            format_step_value_expr(then_branch),
            format_step_value_expr(else_branch)
        ),
    }
}

fn format_binary_value_expr<T, F>(
    lhs: &T,
    op: &str,
    rhs: &T,
    format: F,
) -> String
where
    F: Fn(&T) -> String,
{
    format!("({} {} {})", format(lhs), op, format(rhs))
}

fn format_comparison(lhs: &str, op: &str, rhs: &str) -> String {
    format!(
        "{} {} {}",
        normalize_tokens(lhs),
        op,
        normalize_tokens(rhs)
    )
}

fn format_match(value: &str, pattern: &str) -> String {
    format!(
        "{} matches {}",
        normalize_tokens(value),
        normalize_tokens(pattern)
    )
}

fn format_quantifier(kind: QuantifierKind, domain: &str, body: &str) -> String {
    format!(
        "{} {}: {}",
        quantifier_kind_text(kind),
        normalize_tokens(domain),
        normalize_tokens(body)
    )
}

fn format_joined_bool<T>(op: &str, parts: &[T], format: fn(&T) -> String) -> String {
    let rendered = parts.iter().map(format).collect::<Vec<_>>();
    if rendered.is_empty() {
        "true".to_owned()
    } else if rendered.len() == 1 {
        rendered[0].clone()
    } else {
        format!("({})", rendered.join(&format!(" {} ", op)))
    }
}

fn comparison_op_text(op: ComparisonOp) -> &'static str {
    match op {
        ComparisonOp::Eq => "==",
        ComparisonOp::Ne => "!=",
        ComparisonOp::Lt => "<",
        ComparisonOp::Le => "<=",
        ComparisonOp::Gt => ">",
        ComparisonOp::Ge => ">=",
    }
}

fn builtin_predicate_op_text(op: BuiltinPredicateOp) -> &'static str {
    match op {
        BuiltinPredicateOp::Contains => "contains",
        BuiltinPredicateOp::SubsetOf => "subset_of",
    }
}

fn quantifier_kind_text(kind: QuantifierKind) -> &'static str {
    match kind {
        QuantifierKind::ForAll => "forall",
        QuantifierKind::Exists => "exists",
    }
}

fn normalize_tokens(input: &str) -> String {
    input
        .replace(" :: ", "::")
        .replace(" < ", "<")
        .replace(" > ", ">")
        .replace(" , ", ", ")
        .replace(" ( ", "(")
        .replace(" ) ", ")")
        .replace(" [ ", "[")
        .replace(" ] ", "]")
        .replace(" & ", "&")
        .replace(" && ", " && ")
        .replace(" || ", " || ")
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FiniteModelDomain, nirvash_expr, nirvash_step_expr, nirvash_transition_program,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq, FiniteModelDomain)]
    enum State {
        Idle,
        Busy,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, FiniteModelDomain)]
    enum Action {
        Start,
        Stop,
    }

    fn demo_program() -> TransitionProgram<State, Action> {
        nirvash_transition_program! {
            rule start when matches!(action, Action::Start) && matches!(prev, State::Idle) => {
                set self <= State::Busy;
            }

            rule stop when matches!(action, Action::Stop) && matches!(prev, State::Busy) => {
                set self <= State::Idle;
            }
        }
    }

    #[test]
    fn build_structure_case_collects_action_patterns_and_writes() {
        let structure = build_transition_doc_structure_case("default", &demo_program());
        assert_eq!(structure.rules.len(), 2);
        assert_eq!(
            structure.action_patterns,
            vec!["Action::Start".to_owned(), "Action::Stop".to_owned()]
        );
        assert_eq!(structure.writes, vec!["self".to_owned()]);
        assert!(structure.rules[0].guard.contains("action matches Action::Start"));
    }

    #[test]
    fn format_update_value_expr_handles_if_else() {
        let expr = UpdateValueExprAst::if_else(
            GuardExpr::matches_variant(
                "start",
                "action",
                "Action::Start",
                |_prev: &State, action: &Action| matches!(action, Action::Start),
            ),
            UpdateValueExprAst::literal("State::Busy"),
            UpdateValueExprAst::literal("State::Idle"),
        );
        assert_eq!(
            format_update_value_expr(&expr),
            "if action matches Action::Start then State::Busy else State::Idle"
        );
    }

    #[test]
    fn format_bool_and_step_exprs_prefer_ast_surface() {
        let bool_expr = nirvash_expr!(ready(state) => matches!(state, State::Idle));
        let step_expr = nirvash_step_expr!(start(prev, action, next) =>
            matches!(prev, State::Idle)
                && matches!(action, Action::Start)
                && matches!(next, State::Busy)
        );

        assert!(format_bool_expr(&bool_expr).contains("state matches State::Idle"));
        assert!(format_step_expr(&step_expr).contains("action matches Action::Start"));
    }
}
