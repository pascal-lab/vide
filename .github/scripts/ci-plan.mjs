const FAILED_CONCLUSIONS = new Set(["failure", "timed_out", "startup_failure"]);

const DEFAULT_RUST_TEST_MATRIX = {
  os: ["ubuntu-latest", "windows-latest", "macos-latest"],
};
const FALLBACK_RUST_TEST_MATRIX = { os: ["ubuntu-latest"] };

export async function planCi({ github, context, core, filters }) {
  const eventName = context.eventName;
  const workflowDispatch = eventName === "workflow_dispatch";
  const pullRequest = eventName === "pull_request";
  const failedJobNames = await getPreviousFailedJobNames({
    github,
    context,
    core,
  });

  const rustChanged = filters.rust;
  const vscodeChanged = filters.vscode;
  const packageChanged = filters.package;

  const failedRustTestOs = unique(
    failedJobNames.flatMap((name) => {
      const match = /^Rust Tests \((.+)\)$/.exec(name);
      return match ? [match[1]] : [];
    }),
  );

  let runRustTest = false;
  let rustTestMatrix = FALLBACK_RUST_TEST_MATRIX;
  if (workflowDispatch || rustChanged) {
    runRustTest = true;
    rustTestMatrix = DEFAULT_RUST_TEST_MATRIX;
  } else if (failedRustTestOs.length > 0) {
    runRustTest = true;
    rustTestMatrix = { os: failedRustTestOs };
  }

  let runDevArtifacts = false;

  if (!pullRequest) {
    if (workflowDispatch || packageChanged) {
      runDevArtifacts = true;
    } else {
      runDevArtifacts = failedJobNames.some((name) =>
        name.startsWith("Dev Artifacts"),
      );
    }
  }

  return {
    run_rust_lint:
      workflowDispatch ||
      rustChanged ||
      failedJobNames.includes("Rust Linting"),
    run_rust_test: runRustTest,
    rust_test_matrix: rustTestMatrix,
    run_vscode_extension:
      workflowDispatch ||
      vscodeChanged ||
      failedJobNames.includes("VS Code Checks"),
    run_vscode_web_smoke:
      workflowDispatch ||
      vscodeChanged ||
      packageChanged ||
      failedJobNames.includes("VS Code Web Smoke"),
    run_dev_artifacts: runDevArtifacts,
  };
}

async function getPreviousFailedJobNames({ github, context, core }) {
  if (context.eventName === "workflow_dispatch") {
    return [];
  }

  const previousRun = await getPreviousRun({ github, context });
  if (!previousRun) {
    core.info(`No previous completed CI run found for ${context.eventName}.`);
    return [];
  }

  if (!FAILED_CONCLUSIONS.has(previousRun.conclusion)) {
    core.info(
      `Previous run concluded with ${previousRun.conclusion}; no failed jobs to carry forward.`,
    );
    return [];
  }

  const { owner, repo } = context.repo;
  const jobs = await github.paginate(
    github.rest.actions.listJobsForWorkflowRun,
    {
      owner,
      repo,
      run_id: previousRun.id,
      per_page: 100,
    },
  );

  return unique(
    jobs
      .filter((job) => FAILED_CONCLUSIONS.has(job.conclusion))
      .map((job) => job.name),
  );
}

async function getPreviousRun({ github, context }) {
  const { owner, repo } = context.repo;
  const runQuery = {
    owner,
    repo,
    workflow_id: "ci.yml",
    event: context.eventName,
    status: "completed",
    per_page: context.eventName === "pull_request" ? 100 : 20,
  };

  if (context.eventName === "push") {
    runQuery.branch = context.ref.replace("refs/heads/", "");
  }

  const {
    data: { workflow_runs: runs },
  } = await github.rest.actions.listWorkflowRuns(runQuery);

  return runs.find((run) => {
    if (run.id === context.runId) {
      return false;
    }

    if (context.eventName === "pull_request") {
      const prNumber = context.payload.pull_request?.number;
      return run.pull_requests?.some(
        (pullRequest) => pullRequest.number === prNumber,
      );
    }

    return context.eventName === "push";
  });
}

function failedTargets(jobNames, targets) {
  return unique(
    jobNames.flatMap((name) => {
      const match = /^Dev Package \((.+)\)$/.exec(name);
      return match && targets.has(match[1]) ? [match[1]] : [];
    }),
  ).map((target) => targets.get(target));
}

function unique(values) {
  return [...new Set(values)];
}
