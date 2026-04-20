export interface EncodingProfile {
  id: number;
  name: string;
  codec: string;
  bitrate?: string;
  sample_rate?: number;
  channels?: number;
  bit_depth?: number;
  advanced_args?: string;
  created_at: string;
}

export interface UpsertEncodingProfile {
  name: string;
  codec: string;
  bitrate?: string;
  sample_rate?: number;
  channels?: number;
  bit_depth?: number;
  advanced_args?: string;
}
