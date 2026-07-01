export type Slug = string;

export type Lifecycle = "live" | "planned" | "deprecated";
export type ElementType = "actor" | "system" | "container" | "component";
export type ContainerKind = "service" | "app" | "store" | "queue" | "worker" | "job";
export type SourceKind = "authored" | "generated" | "merged";
export type ValidationSeverity = "error" | "warning";
export type ValidationStage = "parse" | "schema" | "semantic" | "scan";

export interface RepoHandle {
  id: string;
  rootPath: string;
  name: string;
  vcs?: string | null;
  headSha?: string | null;
}

export interface BaseElement {
  slug: Slug;
  name: string;
  description?: string | null;
  tech?: string | null;
  status: Lifecycle;
  code?: string | null;
  generated: boolean;
}

export interface Actor {
  slug: Slug;
  name: string;
  description?: string | null;
  tech?: string | null;
  status: Lifecycle;
  code?: string | null;
  generated: boolean;
}

export interface Component extends BaseElement {}

export interface Container extends BaseElement {
  kind: ContainerKind;
  components: Record<Slug, Component>;
}

export interface System extends BaseElement {
  external: boolean;
  containers: Record<Slug, Container>;
}

export interface Relationship {
  from: Slug;
  to: Slug;
  description: string;
  tech?: string | null;
  status: Lifecycle;
  generated: boolean;
}

export interface ArchitectureModel {
  name: string;
  description?: string | null;
  actors: Record<Slug, Actor>;
  systems: Record<Slug, System>;
  relationships: Relationship[];
  generated: boolean;
}

export interface ElementNode extends BaseElement {
  type: ElementType;
  parentSlug?: Slug | null;
  systemSlug?: Slug | null;
  containerSlug?: Slug | null;
  external?: boolean | null;
  kind?: ContainerKind | null;
  source: SourceKind;
}

export interface ValidationIssue {
  severity: ValidationSeverity;
  stage: ValidationStage;
  code: string;
  message: string;
  path?: string | null;
  line?: number | null;
  column?: number | null;
}

export interface ValidationReport {
  ok: boolean;
  sourceSha?: string | null;
  issues: ValidationIssue[];
}

export interface ScanSummary {
  repo: RepoHandle;
  scanToken: string;
  scannedFiles: number;
  changedFiles: number;
  deletedFiles: number;
  symbols: number;
  imports: number;
  durationMs: number;
  warnings: ValidationIssue[];
}

export interface CodeRef {
  elementSlug: Slug;
  path: string;
  absolutePath: string;
  language?: string | null;
  snippet?: string | null;
}

export interface EffectiveModel {
  repo: RepoHandle;
  sourceSha: string;
  authoredPath?: string | null;
  generatedPath?: string | null;
  model: ArchitectureModel;
  elementsBySlug: Record<Slug, ElementNode>;
  relationships: Relationship[];
  validation: ValidationReport;
}
