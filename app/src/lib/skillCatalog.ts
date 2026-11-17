import type { TranslationKey } from './i18n.js';

export type SkillDescKey = Extract<TranslationKey, `skill${string}Desc`>;

export interface Skill {
  name: string;
  descKey: SkillDescKey;
  special?: boolean;
  pipeline?: boolean;
}

export const SKILLS: Skill[] = [
  { name: '_dispatch', descKey: 'skillDispatchDesc', special: true },
  { name: 'discover', descKey: 'skillDiscoverDesc', pipeline: true },
  { name: 'spec', descKey: 'skillSpecDesc', pipeline: true },
  { name: 'go', descKey: 'skillGoDesc', pipeline: true },
  { name: 'audit', descKey: 'skillAuditDesc', pipeline: true },
  { name: 'eval', descKey: 'skillEvalDesc', pipeline: true },
  { name: 'ship', descKey: 'skillShipDesc', pipeline: true },
  { name: 'orbit', descKey: 'skillOrbitDesc', pipeline: true },
  { name: 'evolve', descKey: 'skillEvolveDesc', pipeline: true },
  { name: 'team', descKey: 'skillTeamDesc', pipeline: true },
  { name: 'tdd', descKey: 'skillTddDesc' },
  { name: 'debug', descKey: 'skillDebugDesc' },
  { name: 'secure', descKey: 'skillSecureDesc' },
  { name: 'threat-model', descKey: 'skillThreatModelDesc' },
  { name: 'vuln-scan', descKey: 'skillVulnScanDesc' },
  { name: 'triage', descKey: 'skillTriageDesc' },
  { name: 'verify', descKey: 'skillVerifyDesc' },
  { name: 'simplify', descKey: 'skillSimplifyDesc' },
  { name: 'perf', descKey: 'skillPerfDesc' },
  { name: 'document', descKey: 'skillDocumentDesc' },
  { name: 'commit', descKey: 'skillCommitDesc' },
  { name: 'context', descKey: 'skillContextDesc' },
  { name: 'council', descKey: 'skillCouncilDesc' },
  { name: 'reflect', descKey: 'skillReflectDesc' },
  { name: 'orchestrate', descKey: 'skillOrchestrateDesc' },
  { name: 'agent-introspection', descKey: 'skillAgentIntrospectionDesc' },
];
