//! eval/scaffold.rs — Generate stack-appropriate benchmark files.
//!
//! `epic eval --scaffold` creates runnable benchmark stubs in `benchmarks/`
//! when no evaluation infrastructure is detected. The generated files:
//!
//! - Are immediately runnable (with the right toolchain installed)
//! - Output `{"composite": 0.0–1.0, ...}` to stdout where the framework allows,
//!   or exit 0/1 for frameworks with fixed output formats
//! - Include TODO markers showing exactly what to replace with real logic

use std::path::{Path, PathBuf};

pub struct ScaffoldResult {
    pub created: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
    pub stack: String,
}

/// Generate benchmark stub(s) for the detected stack.
pub fn scaffold_benchmarks(cwd: &Path, stack: &str) -> Result<ScaffoldResult, String> {
    let mut result = ScaffoldResult {
        created: vec![],
        skipped: vec![],
        stack: stack.to_string(),
    };

    let bench_dir = cwd.join("benchmarks");
    std::fs::create_dir_all(&bench_dir)
        .map_err(|e| format!("create benchmarks/: {e}"))?;

    let tasks: &[(&str, &str, &str)] = match stack {
        "rust" => &[
            ("benches/eval_harness.rs", RUST_BENCH, "benches/"),
        ],
        "python" => &[
            ("benchmarks/eval_runner.py", PYTHON_BENCH, "benchmarks/"),
        ],
        "node" => &[
            ("benchmarks/eval.mjs", NODE_BENCH, "benchmarks/"),
        ],
        "typescript" => &[
            ("benchmarks/eval.ts", TS_BENCH, "benchmarks/"),
        ],
        "go" => &[
            ("benchmarks/eval_test.go", GO_BENCH, "benchmarks/"),
        ],
        "java" => &[
            ("benchmarks/EvalBenchmark.java", JAVA_BENCH, "benchmarks/"),
        ],
        "kotlin" => &[
            ("benchmarks/EvalBenchmark.kt", KOTLIN_BENCH, "benchmarks/"),
        ],
        "ruby" => &[
            ("benchmarks/eval_benchmark.rb", RUBY_BENCH, "benchmarks/"),
        ],
        "php" => &[
            ("benchmarks/eval_benchmark.php", PHP_BENCH, "benchmarks/"),
        ],
        "csharp" => &[
            ("Benchmarks/EvalBenchmark.cs", CSHARP_BENCH, "Benchmarks/"),
        ],
        "swift" => &[
            ("benchmarks/EvalBenchmark.swift", SWIFT_BENCH, "benchmarks/"),
        ],
        "elixir" => &[
            ("benchmarks/eval_benchmark.exs", ELIXIR_BENCH, "benchmarks/"),
        ],
        "cpp" => &[
            ("benchmarks/eval_benchmark.cpp", CPP_BENCH, "benchmarks/"),
        ],
        _ => {
            eprintln!("warning: no scaffold template for stack '{stack}' — writing generic shell script");
            &[("benchmarks/eval.sh", SHELL_BENCH, "benchmarks/")]
        }
    };

    for (rel_path, template, subdir) in tasks {
        let abs_path = cwd.join(rel_path);
        if let Some(parent) = abs_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create {subdir}: {e}"))?;
        }
        if abs_path.exists() {
            result.skipped.push(abs_path);
            continue;
        }
        std::fs::write(&abs_path, template)
            .map_err(|e| format!("write {rel_path}: {e}"))?;
        result.created.push(abs_path);
    }

    Ok(result)
}

// ── Templates ────────────────────────────────────────────────────────

const RUST_BENCH: &str = r#"//! eval_harness.rs — criterion benchmark suite
//!
//! Add to Cargo.toml:
//!   [dev-dependencies]
//!   criterion = { version = "0.5", features = ["html_reports"] }
//!
//!   [[bench]]
//!   name = "eval_harness"
//!   harness = false
//!
//! Run: cargo bench --bench eval_harness
//! epic eval command: `cargo bench --bench eval_harness 2>&1`

use criterion::{black_box, criterion_group, criterion_main, Criterion};

// TODO: replace with your actual crate imports
// use my_crate::core_function;

fn bench_core(c: &mut Criterion) {
    c.bench_function("core_operation", |b| {
        b.iter(|| {
            // TODO: call your main domain function here
            black_box(42 + 1)
        })
    });
}

fn bench_throughput(c: &mut Criterion) {
    use criterion::Throughput;
    let mut group = c.benchmark_group("throughput");
    for size in [100usize, 1_000, 10_000] {
        let input: Vec<u64> = (0..size as u64).collect();
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(format!("process/{size}"), &input, |b, data| {
            b.iter(|| {
                // TODO: replace with real processing
                data.iter().map(|x| black_box(x * 2)).sum::<u64>()
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_core, bench_throughput);
criterion_main!(benches);
"#;

const PYTHON_BENCH: &str = r#"#!/usr/bin/env python3
"""eval_runner.py — Domain benchmark runner.

Outputs {"composite": 0.0-1.0, "details": {...}} to stdout on the last line.
Run: python3 benchmarks/eval_runner.py [full|quick]
epic eval result_type: composite
"""
import json
import sys
import time

# TODO: import your actual modules
# from mypackage import core_function, search_engine


def bench_core_operation(n: int = 1000) -> dict:
    """Benchmark the primary domain operation."""
    start = time.perf_counter()
    errors = 0

    for i in range(n):
        try:
            # TODO: replace with your real function call
            result = sum(range(i))  # placeholder
            assert result >= 0
        except Exception:
            errors += 1

    elapsed = time.perf_counter() - start
    pass_rate = 1.0 - errors / n
    return {
        "pass_rate": round(pass_rate, 4),
        "latency_ms": round(elapsed * 1000 / n, 3),
        "errors": errors,
    }


def bench_precision_recall() -> dict:
    """TODO: Replace with domain-specific precision/recall measurement."""
    # Example structure — replace with real eval set
    expected = [1, 2, 3, 4, 5]
    got = [1, 2, 3]  # TODO: call your model/search here

    tp = len(set(expected) & set(got))
    precision = tp / len(got) if got else 0.0
    recall = tp / len(expected) if expected else 0.0
    f1 = 2 * precision * recall / (precision + recall) if (precision + recall) > 0 else 0.0

    return {"precision": round(precision, 4), "recall": round(recall, 4), "f1": round(f1, 4)}


def main(mode: str = "full") -> None:
    n = 1000 if mode == "full" else 100

    core = bench_core_operation(n)
    pr = bench_precision_recall()

    # Composite: weighted average — tune weights to your domain
    composite = round(
        0.4 * core["pass_rate"]
        + 0.3 * pr["precision"]
        + 0.3 * pr["recall"],
        4,
    )

    output = {
        "composite": composite,
        "details": {
            "core": core,
            "precision_recall": pr,
        },
    }
    print(json.dumps(output))
    sys.exit(0 if composite >= 0.6 else 1)


if __name__ == "__main__":
    mode = sys.argv[1] if len(sys.argv) > 1 else "full"
    main(mode)
"#;

const NODE_BENCH: &str = r#"// eval.mjs — Domain benchmark runner
// Outputs {"composite": 0.0-1.0, ...} to stdout on the last line.
// Run: node benchmarks/eval.mjs [full|quick]
// epic eval result_type: composite

// TODO: import your actual modules
// import { coreFunction } from '../src/index.js';

function bench(fn, n = 1000) {
  const start = performance.now();
  let errors = 0;
  for (let i = 0; i < n; i++) {
    try { fn(i); } catch { errors++; }
  }
  const ms = performance.now() - start;
  return { passRate: 1 - errors / n, latencyMs: ms / n, errors };
}

function benchPrecisionRecall() {
  // TODO: replace with real eval set + model call
  const expected = [1, 2, 3, 4, 5];
  const got = [1, 2, 3]; // placeholder
  const tp = expected.filter(x => got.includes(x)).length;
  const precision = got.length ? tp / got.length : 0;
  const recall = expected.length ? tp / expected.length : 0;
  const f1 = precision + recall ? 2 * precision * recall / (precision + recall) : 0;
  return { precision, recall, f1 };
}

const mode = process.argv[2] ?? 'full';
const n = mode === 'full' ? 1000 : 100;

// TODO: replace placeholder with real domain function
const core = bench(i => i * 2 + 1, n);
const pr = benchPrecisionRecall();

const composite = +(0.4 * core.passRate + 0.3 * pr.precision + 0.3 * pr.recall).toFixed(4);

console.log(JSON.stringify({ composite, details: { core, precisionRecall: pr } }));
process.exit(composite >= 0.6 ? 0 : 1);
"#;

const TS_BENCH: &str = r#"// eval.ts — Domain benchmark runner (TypeScript)
// Compile: npx tsc benchmarks/eval.ts --outDir dist --esModuleInterop
// Run: node dist/eval.js [full|quick]
// Or with ts-node: npx ts-node benchmarks/eval.ts
// epic eval result_type: composite

// TODO: import your actual modules
// import { coreFunction } from '../src/index';

interface BenchResult {
  passRate: number;
  latencyMs: number;
  errors: number;
}

interface PRResult {
  precision: number;
  recall: number;
  f1: number;
}

function bench(fn: (i: number) => unknown, n = 1000): BenchResult {
  const start = performance.now();
  let errors = 0;
  for (let i = 0; i < n; i++) {
    try { fn(i); } catch { errors++; }
  }
  const ms = performance.now() - start;
  return { passRate: 1 - errors / n, latencyMs: +(ms / n).toFixed(3), errors };
}

function benchPrecisionRecall(): PRResult {
  // TODO: replace with real eval set + model call
  const expected = [1, 2, 3, 4, 5];
  const got = [1, 2, 3];
  const tp = expected.filter(x => got.includes(x)).length;
  const precision = got.length ? tp / got.length : 0;
  const recall = expected.length ? tp / expected.length : 0;
  const f1 = precision + recall ? 2 * precision * recall / (precision + recall) : 0;
  return { precision: +precision.toFixed(4), recall: +recall.toFixed(4), f1: +f1.toFixed(4) };
}

const mode = process.argv[2] ?? 'full';
const n = mode === 'full' ? 1000 : 100;

const core = bench(i => i * 2 + 1, n); // TODO: replace
const pr = benchPrecisionRecall();

const composite = +(0.4 * core.passRate + 0.3 * pr.precision + 0.3 * pr.recall).toFixed(4);

console.log(JSON.stringify({ composite, details: { core, precisionRecall: pr } }));
process.exit(composite >= 0.6 ? 0 : 1);
"#;

const GO_BENCH: &str = r#"// Package benchmarks provides domain evaluation benchmarks.
// Run: go test ./benchmarks/ -bench=. -benchmem -count=1
// epic eval result_type: exit_code
package benchmarks

import (
	"encoding/json"
	"fmt"
	"os"
	"testing"
	"time"

	// TODO: import your actual package
	// mypackage "github.com/yourorg/yourrepo/pkg"
)

// BenchmarkCoreOperation benchmarks the primary domain function.
func BenchmarkCoreOperation(b *testing.B) {
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		// TODO: replace with your real function call
		_ = i * 2
	}
}

// BenchmarkThroughput measures processing throughput.
func BenchmarkThroughput(b *testing.B) {
	sizes := []int{100, 1000, 10000}
	for _, n := range sizes {
		b.Run(fmt.Sprintf("n=%d", n), func(b *testing.B) {
			data := make([]int, n)
			for i := range data {
				data[i] = i
			}
			b.ResetTimer()
			for i := 0; i < b.N; i++ {
				// TODO: replace with real processing
				sum := 0
				for _, v := range data {
					sum += v
				}
				_ = sum
			}
		})
	}
}

// TestEvalComposite runs a quick quality check and outputs a composite score.
// Run standalone: go test ./benchmarks/ -run TestEvalComposite -v
func TestEvalComposite(t *testing.T) {
	start := time.Now()
	errors := 0
	n := 1000

	for i := 0; i < n; i++ {
		// TODO: replace with your real function call + assertion
		if i < 0 {
			errors++
		}
	}

	passRate := 1.0 - float64(errors)/float64(n)
	composite := passRate // TODO: combine with other metrics

	result := map[string]interface{}{
		"composite": composite,
		"details": map[string]interface{}{
			"pass_rate":  passRate,
			"latency_ms": float64(time.Since(start).Milliseconds()) / float64(n),
		},
	}

	enc := json.NewEncoder(os.Stdout)
	if err := enc.Encode(result); err != nil {
		t.Fatal(err)
	}

	if composite < 0.6 {
		t.Fatalf("composite score %.4f below threshold 0.6", composite)
	}
}
"#;

const JAVA_BENCH: &str = r#"// EvalBenchmark.java — Domain evaluation benchmark (JMH)
//
// Dependencies (Maven):
//   <dependency>
//     <groupId>org.openjdk.jmh</groupId>
//     <artifactId>jmh-core</artifactId>
//     <version>1.37</version>
//     <scope>test</scope>
//   </dependency>
//
// Run: mvn test -Dtest=EvalBenchmark
// epic eval result_type: exit_code
package benchmarks;

import org.openjdk.jmh.annotations.*;
import org.openjdk.jmh.runner.Runner;
import org.openjdk.jmh.runner.RunnerException;
import org.openjdk.jmh.runner.options.*;

import java.util.concurrent.TimeUnit;

// TODO: import your actual classes
// import com.example.CoreService;

@BenchmarkMode(Mode.AverageTime)
@OutputTimeUnit(TimeUnit.MICROSECONDS)
@State(Scope.Benchmark)
@Warmup(iterations = 3, time = 1)
@Measurement(iterations = 5, time = 1)
@Fork(1)
public class EvalBenchmark {

    // TODO: instantiate your service
    // private CoreService service;

    @Setup
    public void setup() {
        // TODO: initialize service = new CoreService();
    }

    @Benchmark
    public Object benchCoreOperation() {
        // TODO: replace with your real domain call
        return Math.sqrt(42.0);
    }

    @Benchmark
    @OperationsPerInvocation(1000)
    public long benchThroughput() {
        long sum = 0;
        for (int i = 0; i < 1000; i++) {
            // TODO: replace with real processing
            sum += i;
        }
        return sum;
    }

    public static void main(String[] args) throws RunnerException {
        Options opt = new OptionsBuilder()
            .include(EvalBenchmark.class.getSimpleName())
            .build();
        new Runner(opt).run();
    }
}
"#;

const KOTLIN_BENCH: &str = r#"// EvalBenchmark.kt — Domain evaluation benchmark (kotlinx-benchmark)
//
// build.gradle.kts:
//   plugins { id("org.jetbrains.kotlinx.benchmark") version "0.4.11" }
//   dependencies { implementation("org.jetbrains.kotlinx:kotlinx-benchmark-runtime:0.4.11") }
//
// Run: ./gradlew benchmark
// epic eval result_type: exit_code
package benchmarks

import kotlinx.benchmark.*
import java.util.concurrent.TimeUnit

// TODO: import your actual classes
// import com.example.CoreService

@State(Scope.Benchmark)
@BenchmarkMode(Mode.AverageTime)
@OutputTimeUnit(TimeUnit.MICROSECONDS)
@Warmup(iterations = 3)
@Measurement(iterations = 5)
open class EvalBenchmark {

    // TODO: lateinit var service: CoreService

    @Setup
    fun setup() {
        // TODO: service = CoreService()
    }

    @Benchmark
    fun benchCoreOperation(): Double {
        // TODO: replace with real domain call
        return Math.sqrt(42.0)
    }

    @Benchmark
    fun benchThroughput(): Long {
        var sum = 0L
        for (i in 0 until 1000) {
            // TODO: replace with real processing
            sum += i
        }
        return sum
    }
}

// Simple composite scorer — run with: kotlinc -script benchmarks/eval_score.kts
fun main() {
    val results = mapOf("core_us" to 1.5, "throughput_us" to 0.8)
    val composite = if (results.values.all { it < 100.0 }) 1.0 else 0.5
    println("""{"composite": $composite, "details": $results}""")
}
"#;

const RUBY_BENCH: &str = r#"#!/usr/bin/env ruby
# eval_benchmark.rb — Domain evaluation benchmark
# Outputs {"composite": 0.0-1.0, ...} to stdout.
# Run: ruby benchmarks/eval_benchmark.rb [full|quick]
# epic eval result_type: composite
#
# Optional: gem install benchmark-ips

require 'json'
require 'benchmark'

# TODO: require your actual files
# require_relative '../lib/my_module'

def bench_core_operation(n = 1000)
  errors = 0
  elapsed = Benchmark.realtime do
    n.times do |i|
      begin
        # TODO: replace with real domain call
        raise "fail" if i < 0
      rescue
        errors += 1
      end
    end
  end
  pass_rate = 1.0 - errors.to_f / n
  { pass_rate: pass_rate.round(4), latency_ms: (elapsed * 1000 / n).round(3), errors: errors }
end

def bench_precision_recall
  # TODO: replace with real eval set + model call
  expected = [1, 2, 3, 4, 5]
  got = [1, 2, 3]
  tp = (expected & got).length
  precision = got.empty? ? 0.0 : tp.to_f / got.length
  recall    = expected.empty? ? 0.0 : tp.to_f / expected.length
  f1 = (precision + recall).zero? ? 0.0 : 2 * precision * recall / (precision + recall)
  { precision: precision.round(4), recall: recall.round(4), f1: f1.round(4) }
end

mode = ARGV[0] || 'full'
n    = mode == 'full' ? 1000 : 100

core = bench_core_operation(n)
pr   = bench_precision_recall

composite = (0.4 * core[:pass_rate] + 0.3 * pr[:precision] + 0.3 * pr[:recall]).round(4)

puts JSON.generate({ composite: composite, details: { core: core, precision_recall: pr } })
exit composite >= 0.6 ? 0 : 1
"#;

const PHP_BENCH: &str = r#"<?php
/**
 * eval_benchmark.php — Domain evaluation benchmark
 * Outputs {"composite": 0.0-1.0, ...} to stdout.
 * Run: php benchmarks/eval_benchmark.php [full|quick]
 * epic eval result_type: composite
 */

// TODO: require your actual classes
// require_once __DIR__ . '/../src/MyService.php';

function benchCoreOperation(int $n = 1000): array {
    $errors = 0;
    $start = microtime(true);
    for ($i = 0; $i < $n; $i++) {
        try {
            // TODO: replace with real domain call
            if ($i < 0) throw new \RuntimeException('fail');
        } catch (\Throwable $e) {
            $errors++;
        }
    }
    $elapsed = microtime(true) - $start;
    return [
        'pass_rate'  => round(1.0 - $errors / $n, 4),
        'latency_ms' => round($elapsed * 1000 / $n, 3),
        'errors'     => $errors,
    ];
}

function benchPrecisionRecall(): array {
    // TODO: replace with real eval set + model call
    $expected = [1, 2, 3, 4, 5];
    $got      = [1, 2, 3];
    $tp        = count(array_intersect($expected, $got));
    $precision = count($got)      ? $tp / count($got)      : 0.0;
    $recall    = count($expected) ? $tp / count($expected) : 0.0;
    $f1        = ($precision + $recall) > 0
        ? 2 * $precision * $recall / ($precision + $recall)
        : 0.0;
    return [
        'precision' => round($precision, 4),
        'recall'    => round($recall,    4),
        'f1'        => round($f1,        4),
    ];
}

$mode = $argv[1] ?? 'full';
$n    = $mode === 'full' ? 1000 : 100;

$core = benchCoreOperation($n);
$pr   = benchPrecisionRecall();

$composite = round(0.4 * $core['pass_rate'] + 0.3 * $pr['precision'] + 0.3 * $pr['recall'], 4);

echo json_encode(['composite' => $composite, 'details' => ['core' => $core, 'precision_recall' => $pr]]) . PHP_EOL;
exit($composite >= 0.6 ? 0 : 1);
"#;

const CSHARP_BENCH: &str = r#"// EvalBenchmark.cs — Domain evaluation benchmark (BenchmarkDotNet)
//
// Add to your .csproj:
//   <PackageReference Include="BenchmarkDotNet" Version="0.13.*" />
//
// Run: dotnet run -c Release --project Benchmarks/
// epic eval result_type: exit_code
using BenchmarkDotNet.Attributes;
using BenchmarkDotNet.Running;
using System.Text.Json;

// TODO: using YourNamespace;

[MemoryDiagnoser]
[SimpleJob(warmupCount: 3, iterationCount: 5)]
public class EvalBenchmark
{
    // TODO: private CoreService _service;

    [GlobalSetup]
    public void Setup()
    {
        // TODO: _service = new CoreService();
    }

    [Benchmark]
    public double CoreOperation()
    {
        // TODO: return _service.Process(42);
        return Math.Sqrt(42.0);
    }

    [Benchmark]
    [Arguments(100)]
    [Arguments(1000)]
    [Arguments(10_000)]
    public long Throughput(int n)
    {
        long sum = 0;
        for (int i = 0; i < n; i++)
        {
            // TODO: replace with real processing
            sum += i;
        }
        return sum;
    }
}

// Composite scorer entry point — run with: dotnet script Benchmarks/EvalScore.csx
class EvalScore
{
    static int Main(string[] args)
    {
        int n = args.Length > 0 && args[0] == "quick" ? 100 : 1000;
        int errors = 0;
        var sw = System.Diagnostics.Stopwatch.StartNew();

        for (int i = 0; i < n; i++)
        {
            try
            {
                // TODO: call real domain logic
                if (i < 0) throw new Exception("fail");
            }
            catch { errors++; }
        }
        sw.Stop();

        double passRate = 1.0 - (double)errors / n;
        double composite = passRate; // TODO: combine with other metrics

        var result = new { composite, details = new { passRate, latencyMs = sw.Elapsed.TotalMilliseconds / n } };
        Console.WriteLine(JsonSerializer.Serialize(result));
        return composite >= 0.6 ? 0 : 1;
    }
}
"#;

const SWIFT_BENCH: &str = r#"// EvalBenchmark.swift — Domain evaluation benchmark
// Run: swift benchmarks/EvalBenchmark.swift [full|quick]
// epic eval result_type: composite
import Foundation

// TODO: import your module
// import MyModule

struct BenchResult: Codable {
    let passRate: Double
    let latencyMs: Double
    let errors: Int
}

struct PRResult: Codable {
    let precision: Double
    let recall: Double
    let f1: Double
}

struct Output: Codable {
    let composite: Double
    let details: Details
    struct Details: Codable {
        let core: BenchResult
        let precisionRecall: PRResult
    }
}

func benchCore(n: Int = 1000) -> BenchResult {
    var errors = 0
    let start = Date()
    for i in 0..<n {
        // TODO: replace with real domain call
        if i < 0 { errors += 1 }
    }
    let elapsed = Date().timeIntervalSince(start)
    return BenchResult(
        passRate: 1.0 - Double(errors) / Double(n),
        latencyMs: elapsed * 1000 / Double(n),
        errors: errors
    )
}

func benchPrecisionRecall() -> PRResult {
    // TODO: replace with real eval set + model call
    let expected = [1, 2, 3, 4, 5]
    let got = [1, 2, 3]
    let tp = Set(expected).intersection(Set(got)).count
    let precision = got.isEmpty ? 0.0 : Double(tp) / Double(got.count)
    let recall    = expected.isEmpty ? 0.0 : Double(tp) / Double(expected.count)
    let f1 = (precision + recall) > 0 ? 2 * precision * recall / (precision + recall) : 0.0
    return PRResult(precision: precision, recall: recall, f1: f1)
}

let mode = CommandLine.arguments.dropFirst().first ?? "full"
let n = mode == "full" ? 1000 : 100

let core = benchCore(n: n)
let pr = benchPrecisionRecall()
let composite = 0.4 * core.passRate + 0.3 * pr.precision + 0.3 * pr.recall

let output = Output(composite: composite, details: .init(core: core, precisionRecall: pr))
let encoded = try! JSONEncoder().encode(output)
print(String(data: encoded, encoding: .utf8)!)
exit(composite >= 0.6 ? 0 : 1)
"#;

const ELIXIR_BENCH: &str = r#"# eval_benchmark.exs — Domain evaluation benchmark (Benchee)
#
# Install: mix deps.get (add {:benchee, "~> 1.3"} to mix.exs deps)
# Or run without Benchee: elixir benchmarks/eval_benchmark.exs [full|quick]
# epic eval result_type: composite

# TODO: alias your actual modules
# alias MyApp.CoreService

defmodule EvalRunner do
  def bench_core(n) do
    {errors, elapsed_us} =
      :timer.tc(fn ->
        Enum.reduce(1..n, 0, fn i, errors ->
          try do
            # TODO: replace with real domain call
            _ = i * 2
            errors
          rescue
            _ -> errors + 1
          end
        end)
      end)

    pass_rate = 1.0 - errors / n
    %{pass_rate: Float.round(pass_rate, 4), latency_ms: Float.round(elapsed_us / 1000 / n, 3), errors: errors}
  end

  def bench_precision_recall do
    # TODO: replace with real eval set + model call
    expected = MapSet.new([1, 2, 3, 4, 5])
    got = MapSet.new([1, 2, 3])
    tp = MapSet.intersection(expected, got) |> MapSet.size()
    precision = if MapSet.size(got) > 0, do: tp / MapSet.size(got), else: 0.0
    recall    = if MapSet.size(expected) > 0, do: tp / MapSet.size(expected), else: 0.0
    f1 = if precision + recall > 0, do: 2 * precision * recall / (precision + recall), else: 0.0
    %{precision: Float.round(precision, 4), recall: Float.round(recall, 4), f1: Float.round(f1, 4)}
  end
end

mode = List.first(System.argv()) || "full"
n    = if mode == "full", do: 1000, else: 100

core = EvalRunner.bench_core(n)
pr   = EvalRunner.bench_precision_recall()

composite = Float.round(0.4 * core.pass_rate + 0.3 * pr.precision + 0.3 * pr.recall, 4)

result = %{composite: composite, details: %{core: core, precision_recall: pr}}
IO.puts(Jason.encode!(result))
System.halt(if composite >= 0.6, do: 0, else: 1)
"#;

const CPP_BENCH: &str = r#"// eval_benchmark.cpp — Domain evaluation benchmark (Google Benchmark)
//
// Install: vcpkg install benchmark   or   conan install benchmark/1.8.3
// Compile: g++ -O2 -std=c++17 benchmarks/eval_benchmark.cpp -lbenchmark -lpthread -o benchmarks/eval_bench
// Run:     ./benchmarks/eval_bench --benchmark_format=json
// epic eval result_type: exit_code
#include <benchmark/benchmark.h>
#include <cmath>
#include <numeric>
#include <vector>
#include <iostream>
#include <nlohmann/json.hpp>  // optional: header-only JSON — remove if not using

// TODO: #include "your_module.h"

static void BM_CoreOperation(benchmark::State& state) {
    for (auto _ : state) {
        // TODO: replace with real domain call
        benchmark::DoNotOptimize(std::sqrt(42.0));
    }
}
BENCHMARK(BM_CoreOperation)->Unit(benchmark::kMicrosecond);

static void BM_Throughput(benchmark::State& state) {
    const int n = state.range(0);
    std::vector<int> data(n);
    std::iota(data.begin(), data.end(), 0);
    for (auto _ : state) {
        // TODO: replace with real processing
        benchmark::DoNotOptimize(std::accumulate(data.begin(), data.end(), 0LL));
    }
    state.SetItemsProcessed(state.iterations() * n);
}
BENCHMARK(BM_Throughput)->Range(100, 10'000)->Unit(benchmark::kMicrosecond);

// Composite scorer — compile separately or integrate above
// g++ -O2 -std=c++17 -DEVAL_SCORE benchmarks/eval_benchmark.cpp -o benchmarks/eval_score
#ifdef EVAL_SCORE
int main() {
    int n = 1000, errors = 0;
    for (int i = 0; i < n; ++i) {
        // TODO: call real domain logic
        if (i < 0) ++errors;
    }
    double pass_rate = 1.0 - static_cast<double>(errors) / n;
    double composite = pass_rate; // TODO: combine with other metrics
    std::cout << R"({"composite":)" << composite
              << R"(,"details":{"pass_rate":)" << pass_rate << "}}}\n";
    return composite >= 0.6 ? 0 : 1;
}
#else
BENCHMARK_MAIN();
#endif
"#;

const SHELL_BENCH: &str = r#"#!/usr/bin/env sh
# eval.sh — Generic benchmark runner
# Run: sh benchmarks/eval.sh [full|quick]
# epic eval result_type: composite

MODE=${1:-full}
N=1000
if [ "$MODE" = "quick" ]; then N=100; fi

ERRORS=0

i=0
while [ $i -lt $N ]; do
    # TODO: replace with your actual eval command
    # result=$(your_command "$i") || ERRORS=$((ERRORS + 1))
    i=$((i + 1))
done

PASS_RATE=$(awk "BEGIN {printf \"%.4f\", 1 - $ERRORS / $N}")
COMPOSITE=$PASS_RATE  # TODO: combine with other metrics

printf '{"composite":%s,"details":{"pass_rate":%s,"errors":%d}}\n' \
    "$COMPOSITE" "$PASS_RATE" "$ERRORS"

awk "BEGIN {exit ($COMPOSITE < 0.6)}"
"#;
