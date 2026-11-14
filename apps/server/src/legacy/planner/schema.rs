use super::*;
use serde_json::{json, Value};

/// JSON Schema given to the agent. Every property is required and no extra properties are
/// allowed, so the answer cannot carry anything outside this shape.
pub fn output_schema() -> Value {
    json!({
        "type":"object","additionalProperties":false,
        "required":["summary","modules","dependencies"],
        "properties":{
            "summary":{"type":"string","description":format!("What the plan builds. At most {MAX_SUMMARY} characters.")},
            "modules":{"type":"array","items":{
                "type":"object","additionalProperties":false,
                "required":["id","name","kind","layer","slice","segment","responsibility","interfaces","files"],
                "properties":{
                    "id":{"type":"string","description":"Unique id within this plan."},
                    "name":{"type":"string","description":format!("Module name. At most {MAX_NAME} characters.")},
                    "kind":{"type":"string","enum":["new","modify","existing"],
                        "description":"new: to be created. modify: exists and must change. existing: exists, unchanged, and a new or modified module depends on it."},
                    "layer":{"type":"string","enum":["app","pages","widgets","features","entities","shared"]},
                    "slice":{"type":["string","null"],"description":"Slice path below the layer, e.g. home/assignment. null in app and shared."},
                    "segment":{"type":"string","description":"Segment inside the slice (or inside app/shared), e.g. ui, model, api."},
                    "responsibility":{"type":"string","description":format!("The single responsibility of the module, as one sentence of at most {MAX_RESPONSIBILITY} characters.")},
                    "interfaces":{"type":"array","description":format!("Where the module meets the outside. One entry per thing it offers to other modules, and one per request it sends to the server. At most {MAX_INTERFACES}."),
                        "items":{"type":"object","additionalProperties":false,"required":["name","kind","condition","result","inputs","outputs"],
                            "properties":{
                                "name":{"type":"string","description":"Exported name. For kind server: the request as METHOD /path, e.g. POST /api/jobs."},
                                "kind":{"type":"string","enum":["component","hook","function","store","constant","type","server"],
                                    "description":"component, hook, function, store, constant, type: offered to other modules. server: a request this module sends to the server."},
                                "condition":{"type":"string","description":format!("The condition or trigger of the behavior. Empty when the result always holds. At most {MAX_CONDITION} characters.")},
                                "result":{"type":"string","description":format!("What happens under that condition: one observable result. At most {MAX_RESULT} characters. May be empty for kind type and constant.")},
                                "inputs":{"type":"array","items":{"type":"string"},"description":format!("Props, arguments or request data, each written name: Type with a TypeScript type, e.g. id: string. At most {MAX_PORTS}. Empty when there are none.")},
                                "outputs":{"type":"array","items":{"type":"string"},"description":format!("Return value or response, each a TypeScript type or name: Type, e.g. Promise<boolean>. At most {MAX_PORTS}. Empty when there is none.")}
                            }}},
                    "files":{"type":"array","items":{"type":"string"},"description":format!("File names inside the placement. At most {MAX_FILES}.")}
                }
            }},
            "dependencies":{"type":"array","items":{
                "type":"object","additionalProperties":false,"required":["from","to","uses"],
                "properties":{
                    "from":{"type":"string","description":"id of the module that imports."},
                    "to":{"type":"string","description":"id of the module it imports from."},
                    "uses":{"type":"array","items":{"type":"string"},"description":"Names of the interfaces of the imported module that the importing module uses. At least one."}
                }
            }}
        }
    })
}
