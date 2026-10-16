/**
 * Domain-specific components for agent responses.
 *
 * Each is a shadcn Card (header, content, footer) drawn with the Mac tokens in
 * `styles/agent-content.css`: one accent, hairlines, no gradients, no loops.
 * They are registered in availableComponents in jsx-message-renderer.tsx.
 */

export { WeatherCard } from "./weather-card";
export type { WeatherCardProps } from "./weather-card";
export { FileListCard } from "./file-list-card";
export type { FileListCardProps } from "./file-list-card";
export { MetricList, SystemStatusCard } from "./metric-list";
export type { MetricListProps, Metric } from "./metric-list";
export { ComparisonCard } from "./comparison-card";
export type { ComparisonCardProps } from "./comparison-card";
export { LinkCard } from "./link-card";
export type { LinkCardProps } from "./link-card";
export { ImageCard } from "./image-card";
export type { ImageCardProps } from "./image-card";
export { ListRow } from "./list-row";
export { TaskSummaryCard } from "./task-summary-card";
export type { TaskSummaryCardProps, TaskItem, TaskAction } from "./task-summary-card";
// Live components (bound to real state) live in their own files.
export { AgendaCard } from "./agenda-card";
export { MessageCard } from "./message-card";
export { PermissionAskCard } from "./permission-ask-card";
export type { PermissionAskCardProps } from "./permission-ask-card";
export type { MessageCardProps, MessageCardState } from "./message-card";
export type { AgendaCardProps, AgendaCardItem } from "./agenda-card";
export { NowPlayingCard, mediaAppLabel } from "./now-playing-card";
export type { NowPlayingCardProps, MediaState, MediaApp } from "./now-playing-card";
