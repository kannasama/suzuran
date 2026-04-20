import { useQuery } from '@tanstack/react-query';
import { tracksApi } from '../api/tracks';

const ORDERED_KEYS = [
  'title', 'artist', 'albumartist', 'album', 'date', 'genre',
  'tracknumber', 'discnumber', 'totaltracks', 'totaldiscs',
  'label', 'catalognumber', 'composer',
  'musicbrainz_recordingid', 'musicbrainz_releaseid',
];

interface Props {
  trackId: number;
  suggestedTags: Record<string, string>;
}

export function TagDiffTable({ trackId, suggestedTags }: Props) {
  const { data: track } = useQuery({
    queryKey: ['track', trackId],
    queryFn: () => tracksApi.getTrack(trackId),
  });

  const current: Record<string, string> = Object.fromEntries(
    Object.entries(track?.tags ?? {}).filter(([, v]) => typeof v === 'string') as [string, string][]
  );

  const extraKeys = Object.keys(suggestedTags).filter(k => !ORDERED_KEYS.includes(k));
  const keys = [
    ...ORDERED_KEYS.filter(k => suggestedTags[k] || current[k]),
    ...extraKeys,
  ];

  if (keys.length === 0) return null;

  return (
    <table className="w-full text-xs border-collapse">
      <thead>
        <tr className="text-text-muted">
          <th className="text-left pb-1 pr-3 w-36 font-normal">Field</th>
          <th className="text-left pb-1 pr-3 font-normal">Current</th>
          <th className="text-left pb-1 font-normal">Suggested</th>
        </tr>
      </thead>
      <tbody>
        {keys.map(key => {
          const cur = current[key] ?? '';
          const sug = suggestedTags[key] ?? '';
          const changed = cur !== sug;
          return (
            <tr key={key} className={changed ? 'bg-yellow-500/5' : ''}>
              <td className="py-px pr-3 font-mono text-text-muted">{key}</td>
              <td className={`py-px pr-3 ${changed ? 'text-text-muted line-through' : ''}`}>
                {cur || <span className="italic text-text-muted">—</span>}
              </td>
              <td className={`py-px ${changed ? 'text-green-400' : ''}`}>
                {sug || <span className="italic text-text-muted">—</span>}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
