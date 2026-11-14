use serde_json::Value;

/// The task for the agent. It describes what to plan and gives it the facts it needs. The shape of
/// the answer is not requested here: it is enforced by the output schema, and the rules are
/// enforced by `planner::validate`, which rejects the answer if any of them is broken.
pub(crate) fn planning_prompt(
    repo: &str,
    source_root: &str,
    instruction: &str,
    current_plan: &Value,
    existing_paths: &[String],
) -> String {
    format!(
        r#"Plan work for a codebase that follows Feature-Sliced Design. Do not change any file.

Repository: {repo}
FSD source root (relative to the repository): {source_root}

Request: {instruction}

Read the code the request touches. Then answer with the modules to build or change. A module is a unit with one responsibility that a developer can implement and review on its own. For each module give its responsibility, its placement (layer, slice, segment), its files, and its interfaces: what it offers to other modules and the requests it sends to the server. Describe behavior only per interface, as a condition and one result. Write inputs and outputs as TypeScript, not as descriptions. Write names and sentences in the language of the request.

Your answer is checked by a program and rejected as a whole if any of these is not true:
- Every module has a name, a one-sentence responsibility, and a placement in one of the six FSD layers. pages, widgets, features and entities need a slice; app and shared have none.
- A module marked "modify" or "existing" is at a placement that exists in the repository (list below), and every file it names exists there. "existing" modules are included only when another module in the plan imports from them.
- Every module names its files. A "new" module names files that do not exist yet. Only client source files (ts, tsx, js, jsx, css, scss, json) can be planned; code outside the FSD layers is not part of a plan.
- Every "new" or "modify" module has at least one interface. An interface has a unique name and a kind.
- Every component, hook, function, store and server interface of a "new" or "modify" module states its "result". "condition" holds the condition or trigger and is empty if the result always holds. Each is one sentence.
- Every input is written name: Type, and every output is a type or name: Type, using TypeScript types (for example id: string, Promise<boolean>, job: JobView | null). Prose is rejected.
- A module has at least one component, hook, function or store. Something that only declares types, with or without constants, is not a module; list its types and its file in the module that uses them.
- An interface is listed by one module only. A module that only re-exports another module (its interfaces have the same inputs and outputs as those of a module it imports) is not a module.
- An output states a type; it never just repeats the interface name.
- A request to the server is an interface of kind "server", named METHOD /path. A module with such an interface is placed in shared/api or depends, directly or indirectly, on a module in shared/api.
- A dependency names, in "uses", at least one interface of the module it imports from, by the exact name listed there. It cannot use a server request of another module.
- An "existing" module lists only the interfaces that some dependency uses.
- A dependency points to a lower layer, or stays inside one slice. There are no cycles, no duplicates, and no dependencies that start at an "existing" module.
- The length and size limits stated in the answer schema hold.

The plan so far (keep these modules unless the request requires changing them):
{current_plan}

Module placements that exist in the repository:
{}"#,
        existing_paths.join("\n")
    )
}
