use super::*;
use factory_plan::Node;
use factory_repository::Entry;
impl Workbench {
    pub(super) fn undo(&mut self, redo: bool) {
        self.history_action = true;
        let (source, target) = if redo {
            (&mut self.redo, &mut self.history)
        } else {
            (&mut self.history, &mut self.redo)
        };
        if let Some(plan) = source.pop() {
            target.push(std::mem::replace(&mut self.session.plan, plan));
            self.dirty = true;
            self.last_edit = Instant::now();
            if !self
                .session
                .plan
                .nodes
                .iter()
                .any(|node| Some(&node.id) == self.selected.as_ref())
            {
                self.selected = None;
            }
        }
    }

    pub(super) fn add_module(&mut self) {
        let node = Node::default();
        self.selected = Some(node.id.clone());
        self.session.plan.nodes.push(node);
        self.planning = true;
    }

    pub(super) fn add_entry(&mut self, snapshot: &Snapshot, entry: &Entry) {
        let node = Node::from_repository(snapshot, entry);
        let check = Plan {
            nodes: vec![node.clone()],
            ..Default::default()
        };
        if !check.errors().is_empty() {
            self.notify(
                "슬라이스 내부의 세그먼트·그룹·파일을 선택한 뒤 계획에 추가하세요.",
                true,
            );
            return;
        }
        if self
            .session
            .plan
            .nodes
            .iter()
            .any(|old| old.path == node.path && old.files == node.files)
        {
            self.notify("이 그룹은 이미 계획에 있습니다.", false);
            return;
        }
        self.selected = Some(node.id.clone());
        self.session.plan.nodes.push(node);
        self.planning = true;
        self.notify(
            "실제 파일 목록을 계획에 복사했습니다. 책임과 변경할 내용을 직접 작성하세요.",
            false,
        );
    }
}
