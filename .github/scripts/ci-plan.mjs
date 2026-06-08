const FAILED_CONCLUSIONS = new Set(["failure", "timed_out", "startup_failure"]);

const DEFAULT_RUST_TEST_MATRIX = {
  os: ["ubuntu-latest", "windows-latest", "macos-latest"],
};
const FALLBACK_RUST_TEST_MATRIX = { os: ["ubuntu-latest"] };

const DEV_PACKAGE_TARGETS = new Map([
  ["linux-x64", { os: "ubuntu-22.04", target: "linux-x64" }],
  ["linux-arm64", { os: "ubuntu-22.04-arm", target: "linux-arm64" }],
  ["win32-x64", { os: "windows-latest", target: "win32-x64" }],
  ["darwin-arm64", { os: "macos-15", target: "darwin-arm64" }],
]);

const DEV_ALPINE_TARGETS = new Map([
  [
    "alpine-x64",
    {
      target: "alpine-x64",
      image: "ghcr.io/blackdex/rust-musl:x86_64-musl-nightly",
      "rust-target": "x86_64-unknown-linux-musl",
    },
  ],
]);

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

  let runDevPackage = false;
  let runDevWebPackage = false;
  let runDevAlpinePackage = false;
  let devPackageMatrix = [DEV_PACKAGE_TARGETS.get("linux-x64")];
  let devAlpineMatrix = [DEV_ALPINE_TARGETS.get("alpine-x64")];

  if (!pullRequest) {
    if (workflowDispatch || packageChanged) {
      runDevPackage = true;
      runDevWebPackage = true;
      runDevAlpinePackage = true;
      devPackageMatrix = [...DEV_PACKAGE_TARGETS.values()];
      devAlpineMatrix = [...DEV_ALPINE_TARGETS.values()];
    } else {
      const failedDevPackageTargets = failedTargets(
        failedJobNames,
        DEV_PACKAGE_TARGETS,
      );
      const failedDevAlpineTargets = failedTargets(
        failedJobNames,
        DEV_ALPINE_TARGETS,
      );

      runDevPackage = failedDevPackageTargets.length > 0;
      runDevWebPackage = failedJobNames.includes("Dev Package (web)");
      runDevAlpinePackage = failedDevAlpineTargets.length > 0;

      if (runDevPackage) {
        devPackageMatrix = failedDevPackageTargets;
      }
      if (runDevAlpinePackage) {
        devAlpineMatrix = failedDevAlpineTargets;
      }
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
    run_dev_package: runDevPackage,
    dev_package_matrix: devPackageMatrix,
    run_dev_web_package: runDevWebPackage,
    run_dev_alpine_package: runDevAlpinePackage,
    dev_alpine_matrix: devAlpineMatrix,
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
