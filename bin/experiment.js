#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");

const {
  decideExperiment,
  formatReport,
  runBenchmark,
} = require("./benchmark.js");

async function runExperiment(options) {
  const repoRoot = options.repoRoot || path.resolve(__dirname, "..");
  const configPath = path.resolve(options.config);
  const config = readJson(configPath);
  const suite = readSuite(repoRoot, config.suite);
  const baseline = readBaseline(repoRoot, config.baseline);
  const goals = readJson(path.join(repoRoot, "benchmarks", "goals", "production.json"));

  const resultDir = path.join(repoRoot, "experiments", config.id);
  fs.mkdirSync(resultDir, { recursive: true });

  let benchmarkResult;
  let indexingCrashed = false;
  try {
    benchmarkResult = await runBenchmark({
      repoRoot,
      scale: suite.scale || "quick",
      debug: config.debug || {},
    });
  } catch (error) {
    indexingCrashed = true;
    benchmarkResult = {
      summary: {
        aqi: 0,
        averageRecallAt10: 0,
        averagePrecisionAt10: 0,
        queryLatency: { p95Ms: Number.POSITIVE_INFINITY },
        criticalFailures: Number.POSITIVE_INFINITY,
        permissionLeakage: 0,
        error: error.stack || error.message,
      },
      report: error.stack || error.message,
    };
  }

  const before = baselineToDecisionMetrics(baseline);
  const after = summaryToDecisionMetrics(benchmarkResult.summary);
  const gatedAqi = applyProductionGates(benchmarkResult.summary, goals, { indexingCrashed });
  const decision = indexingCrashed ? "revert" : decideExperiment(before, after);
  const productionChecks = productionGoalChecks(benchmarkResult.summary, goals, { indexingCrashed });

  const result = {
    experiment: {
      id: config.id,
      suite: config.suite,
      baseline: config.baseline,
      behaviorChange: Boolean(config.behaviorChange),
    },
    decision,
    before,
    after,
    gatedAqi,
    productionChecks,
    summary: benchmarkResult.summary,
  };

  const report = benchmarkResult.report || formatReport(benchmarkResult.summary);
  const decisionText = formatDecision({ config, decision, before, after, gatedAqi, productionChecks });

  fs.writeFileSync(path.join(resultDir, "result.json"), `${JSON.stringify(result, null, 2)}\n`);
  fs.writeFileSync(path.join(resultDir, "report.txt"), `${report}\n`);
  fs.writeFileSync(path.join(resultDir, "decision.md"), decisionText);

  return { resultDir, result };
}

function readSuite(repoRoot, suiteName) {
  return readJson(path.join(repoRoot, "benchmarks", "suites", `${suiteName}.json`));
}

function readBaseline(repoRoot, baselineName) {
  return readJson(path.join(repoRoot, "benchmarks", "baselines", `${baselineName}.json`));
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function baselineToDecisionMetrics(baseline) {
  return {
    aqi: Number(baseline.aqi || 0),
    recallAt10: Number(baseline.retrieval?.recallAt10 ?? baseline.retrieval?.averageRecallAtK ?? 0),
    precisionAt10: Number(
      baseline.retrieval?.precisionAt10 ?? baseline.retrieval?.averagePrecisionAtK ?? 0,
    ),
    p95LatencyMs: Number(baseline.queryLatency?.p95Ms || 0),
    criticalFailures: Number(baseline.retrieval?.criticalFailures || 0),
    permissionLeakage: Number(baseline.security?.permissionLeakage || 0),
  };
}

function summaryToDecisionMetrics(summary) {
  return {
    aqi: Number(summary.aqi || 0),
    recallAt10: Number(summary.averageRecallAt10 ?? summary.averageRecallAt5 ?? 0),
    precisionAt10: Number(summary.averagePrecisionAt10 ?? summary.averagePrecisionAt5 ?? 0),
    p95LatencyMs: Number(summary.queryLatency?.p95Ms || 0),
    criticalFailures: Number(summary.criticalFailures || 0),
    permissionLeakage: Number(summary.permissionLeakage || 0),
  };
}

function applyProductionGates(summary, goals, context = {}) {
  const hardGates = goals.hardGates || {};
  let aqi = Number(summary.aqi || 0);
  if (Number(summary.permissionLeakage || 0) > 0) {
    aqi = Math.min(aqi, Number(hardGates.permissionLeakageMaxAqi ?? 50));
  }
  if (Number(summary.criticalFailures || 0) > 0) {
    aqi = Math.min(aqi, Number(hardGates.criticalFailuresMaxAqi ?? 80));
  }
  const sourceCoverage = summary.grounding?.sourceCoverage;
  if (Number.isFinite(sourceCoverage) && sourceCoverage < 0.8) {
    aqi = Math.min(aqi, Number(hardGates.sourceCoverageBelow80MaxAqi ?? 75));
  }
  if (context.indexingCrashed) {
    aqi = Math.min(aqi, Number(hardGates.indexingCrashMaxAqi ?? 70));
  }
  return aqi;
}

function productionGoalChecks(summary, goals, context = {}) {
  return {
    aqi: compare(summary.aqi, goals.aqi?.target, ">="),
    recallAt10: compare(summary.averageRecallAt10, goals.retrieval?.recallAt10, ">="),
    precisionAt10: compare(summary.averagePrecisionAt10, goals.retrieval?.precisionAt10, ">="),
    top3Accuracy: compare(summary.top3Accuracy, goals.retrieval?.top3Accuracy, ">="),
    mrrAt10: compare(summary.mrrAt10, goals.retrieval?.mrrAt10, ">="),
    ndcgAt10: compare(summary.ndcgAt10, goals.retrieval?.ndcgAt10, ">="),
    p95LatencyMs: compare(summary.queryLatency?.p95Ms, goals.latency?.p95Ms, "<="),
    p99LatencyMs: compare(summary.queryLatency?.p99Ms, goals.latency?.p99Ms, "<="),
    criticalFailures: compare(summary.criticalFailures, goals.retrieval?.criticalFailures, "<="),
    permissionLeakage: compare(summary.permissionLeakage, goals.security?.permissionLeakage, "<="),
    indexingCrashed: { actual: Boolean(context.indexingCrashed), target: false, pass: !context.indexingCrashed },
  };
}

function compare(actual, target, op) {
  if (!Number.isFinite(actual) || !Number.isFinite(target)) {
    return { actual: actual ?? null, target: target ?? null, pass: null };
  }
  return {
    actual,
    target,
    pass: op === ">=" ? actual >= target : actual <= target,
  };
}

function formatDecision({ config, decision, before, after, gatedAqi, productionChecks }) {
  return `# Experiment ${config.id} Decision

Decision: ${decision}

## Before

- AQI: ${before.aqi}
- Recall@10: ${before.recallAt10}
- Precision@10: ${before.precisionAt10}
- p95 latency: ${before.p95LatencyMs} ms
- Critical failures: ${before.criticalFailures}
- Permission leakage: ${before.permissionLeakage}

## After

- AQI: ${after.aqi}
- Gated AQI: ${gatedAqi}
- Recall@10: ${after.recallAt10}
- Precision@10: ${after.precisionAt10}
- p95 latency: ${after.p95LatencyMs} ms
- Critical failures: ${after.criticalFailures}
- Permission leakage: ${after.permissionLeakage}

## Production Checks

${Object.entries(productionChecks)
  .map(([name, check]) => `- ${name}: ${check.pass === null ? "n/a" : check.pass ? "pass" : "fail"} (${check.actual} / ${check.target})`)
  .join("\n")}
`;
}

function parseArgs(argv) {
  const options = {};
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--config") {
      options.config = argv[++i];
    } else if (arg === "--help" || arg === "-h") {
      options.help = true;
    } else if (!options.config && arg.endsWith(".json")) {
      options.config = arg;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  return options;
}

function usage() {
  return `Usage: node bin/experiment.js --config <path>
       node bin/experiment.js <path>

Runs a benchmark experiment, compares it to a baseline, applies production gates,
and writes result.json, report.txt, and decision.md into experiments/<id>.
`;
}

if (require.main === module) {
  (async () => {
    try {
      const options = parseArgs(process.argv.slice(2));
      if (options.help || !options.config) {
        process.stdout.write(usage());
        process.exitCode = options.help ? 0 : 1;
        return;
      }
      const { resultDir, result } = await runExperiment(options);
      process.stdout.write(`Experiment ${result.experiment.id}: ${result.decision}\n`);
      process.stdout.write(`Wrote ${resultDir}\n`);
    } catch (error) {
      process.stderr.write(`${error.stack || error.message}\n`);
      process.exitCode = 1;
    }
  })();
}

module.exports = {
  applyProductionGates,
  baselineToDecisionMetrics,
  productionGoalChecks,
  runExperiment,
  summaryToDecisionMetrics,
};
