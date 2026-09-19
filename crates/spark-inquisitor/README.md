# spark-inquisitor

Autonomous Sovereign Inquisitor, PR code reviewer, and issue triage gatekeeper.

## Architecture

`spark-inquisitor` is a pure native compiled Rust binary that enforces constitutional invariants across GitHub repositories:

1. **Zero Telemetry**: Flags tracking scripts, analytics SDKs, and data collection calls.
2. **Sovereign Voice and Anti-Slop**: Flags em dashes, en dashes, and banned AI buzzwords.
3. **Contributor Alignment**: Administers the Sovereign Contributor Oath and interview.
4. **Autonomous Issue Triage**: Classifies bug reports, complaints, proposals, and community inquiries.

## Commands

- `spark-inquisitor review --author <user> --pr <num> --title <title> --diff <file>`: Runs constitutional diff audit and generates GitHub PR review comment.
- `spark-inquisitor triage-issue --author <user> --issue <num> --title <title> --body <file>`: Analyzes issue description and outputs triage guidance with labels.
- `spark-inquisitor evaluate --testimony <file> --author <user>`: Evaluates contributor responses against constitutional criteria.
- `spark-inquisitor doctor`: Runs local self-diagnostics.
