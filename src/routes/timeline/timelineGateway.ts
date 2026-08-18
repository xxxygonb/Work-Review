import { invoke } from '$lib/utils/safeInvoke.ts';

import {
  parseTimelineActivities,
  type TimelineActivity,
} from './timelineData.ts';
import {
  parseHourlySummaryRecords,
  type HourlySummaryRecord,
} from './summaryPresentation.ts';

export interface TimelinePageRequest {
  readonly date: string;
  readonly limit: number;
  readonly offset: number;
}

export type InvokeUnknown = (
  command: string,
  args?: Record<string, unknown>,
) => Promise<unknown>;

export interface TimelineGateway {
  getPage(request: TimelinePageRequest): Promise<TimelineActivity[]>;
  getHourlySummaries(date: string): Promise<HourlySummaryRecord[]>;
}

export function createTimelineGateway(invokeUnknown: InvokeUnknown): TimelineGateway {
  return {
    async getPage(request) {
      const payload = await invokeUnknown('get_timeline', { ...request });
      return parseTimelineActivities(payload);
    },

    async getHourlySummaries(date) {
      const payload = await invokeUnknown('get_hourly_summaries', { date });
      return parseHourlySummaryRecords(payload);
    },
  };
}

const invokeUnknown: InvokeUnknown = (command, args) => invoke<unknown>(command, args);

export const timelineGateway = createTimelineGateway(invokeUnknown);