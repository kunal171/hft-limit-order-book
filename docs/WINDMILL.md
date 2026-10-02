# Windmill Usage

Windmill runs the Rust engine as an external job. The Windmill script changes
into the mounted project directory, calls the wrapper, and returns only
`summary.json` as the job result.

## Wrapper scripts

```bash
cargo build --release

# Fixed output folder
./scripts/run_simulation.sh synthetic-crossing 100 runs/windmill-test

# Fresh timestamped folder per run
./scripts/run_simulation.sh synthetic-crossing 100 auto

# Custom prefix for auto folders
RUN_PREFIX=windmill-scheduled ./scripts/run_simulation.sh synthetic-crossing 100 auto

# Simulation plus deterministic analysis, prints one JSON object
./scripts/run_and_analyze.sh synthetic-crossing 100 auto
```

`run_simulation.sh` uses `target/release/limit_order_book`, creates the output
directory, writes verbose output to `run.log`, and prints only `summary.json`
to stdout. Each run folder contains `events.json`, `snapshot.json`,
`summary.json`, and `run.log`. Everything under `runs/` is ignored by Git.

## Windmill Bash script

```bash
scenario="$1"
count="$2"
output_dir="$3"

# Trim spaces/newlines from Windmill inputs.
scenario="$(echo "$scenario" | xargs)"
count="$(echo "$count" | xargs)"
output_dir="$(echo "$output_dir" | xargs)"

# Defaults if the user leaves inputs empty.
scenario="${scenario:-synthetic-crossing}"
count="${count:-100}"
output_dir="${output_dir:-auto}"

cd /workspace/limit_order_book

RUN_PREFIX=windmill-scheduled ./scripts/run_simulation.sh "$scenario" "$count" "$output_dir"
```

Recommended inputs: `scenario: synthetic-crossing`, `count: 100`,
`output_dir: auto`. A fixed folder is overwritten by every scheduled run;
`auto` gives each run a fresh timestamped folder.

## Schedules

Windmill uses six-field cron expressions. The first field is seconds:

```text
*/5 * * * * *   every 5 seconds
0 */5 * * * *   every 5 minutes
```
