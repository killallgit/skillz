## Context Usage

**Model:** claude-sonnet-5-5  
**Tokens:** 20.1k / 600k (3%)

### Estimated usage by category

| Category | Tokens | Percentage |
|----------|--------|------------|
| System prompt | 2.5k | 0.4% |
| System tools | 668 | 0.1% |
| MCP tools | 624 | 0.1% |
| MCP server instructions | 1.4k | 0.2% |
| MCP tools (deferred) | 226.2k | 37.7% |
| System tools (deferred) | 13.7k | 2.3% |
| Custom agents | 709 | 0.1% |
| Memory files | 384 | 0.1% |
| Skills | 9.8k | 1.6% |
| Messages | 4k | 0.7% |
| Free space | 546.9k | 91.1% |
| Autocompact buffer | 33k | 5.5% |

### Custom Agents

| Agent Type | Source | Tokens |
|------------|--------|--------|
| caveman:cavecrew-builder | Plugin | 136 |
| caveman:cavecrew-investigator | Plugin | 114 |
| caveman:cavecrew-reviewer | Plugin | 124 |
| killall-agents:comment-janitor | Plugin | 194 |
| killall-agents:git-janitor | Plugin | 68 |
| killall-agents:git-janitor-investigator | Plugin | 73 |

### Memory Files

| Type | Path | Tokens |
|------|------|--------|
| User | /Users/ryan/.claude/CLAUDE.md | 233 |
| User | /Users/ryan/.claude/RTK.md | 151 |

### Skills

| Skill | Source | Tokens |
|-------|--------|--------|
| architecture | User | ~100 |
| caveman | User | ~70 |
| code-review | User | ~140 |
| codebase-design | User | ~100 |
| domain-modeling | User | ~60 |
| frontend-design | User | ~70 |
| grilling | User | ~60 |
| research | User | ~30 |
| skill-creator | User | ~110 |
| tdd | User | ~50 |
| writing-for-agents | User | ~30 |
| caveman:caveman-init | Plugin (caveman) | ~40 |
| eyepop-agent-skills:argocd | Plugin (eyepop-agent-skills) | ~150 |
| eyepop-agent-skills:auth | Plugin (eyepop-agent-skills) | ~160 |
| eyepop-agent-skills:cicd | Plugin (eyepop-agent-skills) | ~220 |
| eyepop-agent-skills:cli | Plugin (eyepop-agent-skills) | ~160 |
| eyepop-agent-skills:cloudsql | Plugin (eyepop-agent-skills) | ~150 |
| eyepop-agent-skills:e2e-testing | Plugin (eyepop-agent-skills) | ~160 |
| eyepop-agent-skills:issues | Plugin (eyepop-agent-skills) | ~190 |
| eyepop-agent-skills:logs | Plugin (eyepop-agent-skills) | ~140 |
| eyepop-agent-skills:secrets | Plugin (eyepop-agent-skills) | ~160 |
| caveman:cavecrew | Plugin (caveman) | ~90 |
| caveman:caveman | Plugin (caveman) | ~80 |
| caveman:caveman-commit | Plugin (caveman) | ~50 |
| caveman:caveman-compress | Plugin (caveman) | ~60 |
| caveman:caveman-discover | Plugin (caveman) | ~70 |
| caveman:caveman-evidence-review | Plugin (caveman) | ~70 |
| caveman:caveman-explore | Plugin (caveman) | ~90 |
| caveman:caveman-help | Plugin (caveman) | ~40 |
| caveman:caveman-learn | Plugin (caveman) | ~110 |
| caveman:caveman-manage | Plugin (caveman) | ~60 |
| caveman:caveman-optimize | Plugin (caveman) | ~80 |
| caveman:caveman-review | Plugin (caveman) | ~60 |
| caveman:caveman-setup | Plugin (caveman) | ~70 |
| caveman:caveman-stats | Plugin (caveman) | ~70 |
| caveman:investigate-first | Plugin (caveman) | ~70 |
| caveman:lean-build | Plugin (caveman) | ~70 |
| caveman:migration | Plugin (caveman) | ~60 |
| caveman:safe-refactor | Plugin (caveman) | ~60 |
| caveman:surgical-patch | Plugin (caveman) | ~60 |
| caveman:verify-and-stop | Plugin (caveman) | ~60 |
| killall-skills:catch-up | Plugin (killall-skills) | ~150 |
| killall-skills:check-agent-docs | Plugin (killall-skills) | ~150 |
| killall-skills:follow-the-rules | Plugin (killall-skills) | ~70 |
| killall-skills:git-janitor | Plugin (killall-skills) | ~130 |
| killall-skills:wait-for-action | Plugin (killall-skills) | ~100 |
| ponytail:ponytail | Plugin (ponytail) | ~130 |
| ponytail:ponytail-audit | Plugin (ponytail) | ~120 |
| ponytail:ponytail-debt | Plugin (ponytail) | ~80 |
| ponytail:ponytail-gain | Plugin (ponytail) | ~60 |
| ponytail:ponytail-help | Plugin (ponytail) | ~60 |
| ponytail:ponytail-review | Plugin (ponytail) | ~130 |
| killall-agents:follow-the-rules | Plugin (killall-agents) | ~70 |
| killall-agents:orchestrate | Plugin (killall-agents) | ~130 |
| agentmemory:agentmemory-agents | Plugin (agentmemory) | ~90 |
| agentmemory:agentmemory-architecture | Plugin (agentmemory) | ~100 |
| agentmemory:agentmemory-config | Plugin (agentmemory) | ~90 |
| agentmemory:agentmemory-hooks | Plugin (agentmemory) | ~90 |
| agentmemory:agentmemory-mcp-tools | Plugin (agentmemory) | ~80 |
| agentmemory:agentmemory-rest-api | Plugin (agentmemory) | ~90 |
| agentmemory:commit-context | Plugin (agentmemory) | ~100 |
| agentmemory:commit-history | Plugin (agentmemory) | ~90 |
| agentmemory:forget | Plugin (agentmemory) | ~80 |
| agentmemory:handoff | Plugin (agentmemory) | ~90 |
| agentmemory:lesson | Plugin (agentmemory) | ~90 |
| agentmemory:memory-discipline | Plugin (agentmemory) | ~100 |
| agentmemory:recall | Plugin (agentmemory) | ~90 |
| agentmemory:recap | Plugin (agentmemory) | ~90 |
| agentmemory:remember | Plugin (agentmemory) | ~90 |
| agentmemory:session-history | Plugin (agentmemory) | ~80 |
| agentmemory:write-agentmemory-skill | Plugin (agentmemory) | ~70 |
| proofcast:proofcast | Plugin (proofcast) | ~80 |
| dataviz | Built-in | ~480 |
| update-config | Built-in | ~240 |
| keybindings-help | Built-in | ~80 |
| simplify | Built-in | ~60 |
| fewer-permission-prompts | Built-in | ~60 |
| loop | Built-in | ~120 |
| schedule | Built-in | ~130 |
| claude-api | Built-in | ~360 |
| workflow-authoring | Built-in | ~80 |
| run | Built-in | ~120 |
| plugin-authoring | Built-in | ~90 |
| init | Built-in | ~20 |
| security-review | Built-in | ~30 |
| built-in-browser | claude.ai sync | ~290 |
| chrome-browser | claude.ai sync | ~260 |
| computer-use | claude.ai sync | < 20 |
| anthropic-skills:deep-research | claude.ai sync | ~200 |
| docs | claude.ai sync | ~340 |
| docx | claude.ai sync | < 20 |
| google-workspace | claude.ai sync | < 20 |
| pdf | claude.ai sync | ~150 |
| pptx | claude.ai sync | < 20 |
| xlsx | claude.ai sync | < 20 |

