//! Rendering specific [Stages](Stage)

use crate::context::MapContext;
use crate::schedule::{MultiStage, Schedule, Stage, StageLabel};
use graph_runner_stage::GraphRunnerStage;
use resource_stage::ResourceStage;
use upload_stage::UploadStage;

mod graph_runner_stage;
mod phase_sort_stage;
mod queue_stage;
mod resource_stage;
mod upload_stage;

use crate::multi_stage;
use crate::render::stages::phase_sort_stage::PhaseSortStage;
use crate::render::stages::queue_stage::QueueStage;
pub use graph_runner_stage::{draw_graph, node};

/// The labels of the default App rendering stages.
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum RenderStageLabel {
    /// Prepare render resources from the extracted data for the GPU.
    /// For example during this phase textures are created, buffers are allocated and written.
    Prepare,

    /// Queues [PhaseItems](crate::render::render_phase::draw::PhaseItem) that depend on
    /// [`Prepare`](RenderStageLabel::Prepare) data and queue up draw calls to run during the
    /// [`Render`](RenderStageLabel::Render) stage.
    Queue,

    /// Sort the [`RenderPhases`](crate::render_phase::RenderPhase) here.
    PhaseSort,

    /// Actual rendering happens here.
    /// In most cases, only the render backend should insert resources here.
    Render,

    /// Cleanup render resources here.
    Cleanup,
}

impl StageLabel for RenderStageLabel {
    fn dyn_clone(&self) -> Box<dyn StageLabel> {
        Box::new(self.clone())
    }
}

multi_stage!(PrepareStage, upload: UploadStage, resource: ResourceStage);

pub fn register_render_stages(schedule: &mut Schedule) {
    schedule.add_stage(RenderStageLabel::Prepare, PrepareStage::default());
    schedule.add_stage(RenderStageLabel::Queue, QueueStage::default());
    schedule.add_stage(RenderStageLabel::PhaseSort, PhaseSortStage::default());
    schedule.add_stage(RenderStageLabel::Render, GraphRunnerStage::default());
}
