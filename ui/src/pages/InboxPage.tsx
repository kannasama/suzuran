import { Fragment } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { tagSuggestionsApi } from '../api/tagSuggestions';
import type { TagSuggestion } from '../types/tagSuggestion';
import { TopNav } from '../components/TopNav';

export default function InboxPage() {
  const qc = useQueryClient();

  const { data: suggestions = [], isLoading } = useQuery({
    queryKey: ['tag-suggestions'],
    queryFn: () => tagSuggestionsApi.listPending(),
  });

  const accept = useMutation({
    mutationFn: (id: number) => tagSuggestionsApi.accept(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] });
      qc.invalidateQueries({ queryKey: ['inbox-count'] });
    },
    onError: (err) => {
      console.error('Failed to accept suggestion:', err);
    },
  });

  const reject = useMutation({
    mutationFn: (id: number) => tagSuggestionsApi.reject(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] });
      qc.invalidateQueries({ queryKey: ['inbox-count'] });
    },
    onError: (err) => {
      console.error('Failed to reject suggestion:', err);
    },
  });

  const batchAccept = useMutation({
    mutationFn: () => tagSuggestionsApi.batchAccept(0.8),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tag-suggestions'] });
      qc.invalidateQueries({ queryKey: ['inbox-count'] });
    },
    onError: (err) => {
      console.error('Failed to batch-accept suggestions:', err);
    },
  });

  if (isLoading) {
    return (
      <>
        <TopNav />
        <div className="p-4 text-text-muted text-sm">Loading…</div>
      </>
    );
  }

  return (
    <div className="flex flex-col h-screen bg-bg-base overflow-hidden">
      <TopNav />
      <div className="flex flex-col flex-1 overflow-hidden">
        {/* Toolbar */}
        <div className="flex items-center justify-between px-4 py-2 border-b border-border bg-bg-surface flex-shrink-0">
          <span className="text-sm text-text-muted">
            {suggestions.length === 0
              ? 'No pending suggestions'
              : `${suggestions.length} pending suggestion${suggestions.length !== 1 ? 's' : ''}`}
          </span>
          {suggestions.length > 0 && (
            <button
              onClick={() => batchAccept.mutate()}
              disabled={batchAccept.isPending}
              className="px-3 py-1 text-xs bg-accent text-bg-base rounded
                         hover:opacity-90 disabled:opacity-50"
            >
              Accept all ≥ 80%
            </button>
          )}
        </div>

        {/* Suggestion list */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          {suggestions.length === 0 ? (
            <p className="text-center text-text-muted text-sm pt-12">Inbox is empty</p>
          ) : (
            suggestions.map(s => (
              <SuggestionCard
                key={s.id}
                suggestion={s}
                onAccept={() => accept.mutate(s.id)}
                onReject={() => reject.mutate(s.id)}
                isPending={
                  (accept.isPending && accept.variables === s.id) ||
                  (reject.isPending && reject.variables === s.id)
                }
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}

function SuggestionCard({
  suggestion,
  onAccept,
  onReject,
  isPending,
}: {
  suggestion: TagSuggestion;
  onAccept: () => void;
  onReject: () => void;
  isPending: boolean;
}) {
  const pct = Math.round(suggestion.confidence * 100);

  return (
    <div className="border border-border rounded bg-bg-panel">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-border">
        <div className="flex items-center gap-2">
          <span className="text-xs uppercase tracking-wide text-text-muted font-mono">
            {suggestion.source}
          </span>
          <span
            className={`text-xs font-mono ${pct >= 80 ? 'text-green-400' : 'text-yellow-400'}`}
          >
            {pct}%
          </span>
        </div>
        <span className="text-xs text-text-muted font-mono">
          track #{suggestion.track_id}
        </span>
      </div>

      {/* Cover art + tag table */}
      <div className="flex gap-4 p-4">
        {suggestion.cover_art_url && (
          <img
            src={suggestion.cover_art_url}
            alt="cover"
            className="w-20 h-20 object-cover rounded border border-border flex-shrink-0"
            onError={e => { (e.currentTarget as HTMLImageElement).style.display = 'none'; }}
          />
        )}
        <div className="flex-1 min-w-0">
          {/* Tag list (no diff yet — diff added in Task 9) */}
          <dl className="grid grid-cols-[8rem_1fr] gap-x-2 gap-y-0.5 text-sm">
            {Object.entries(suggestion.suggested_tags).map(([k, v]) => (
              <Fragment key={k}>
                <dt className="text-text-muted font-mono text-xs truncate">{k}</dt>
                <dd className="truncate">{v}</dd>
              </Fragment>
            ))}
          </dl>
        </div>
      </div>

      {/* Actions */}
      <div className="flex gap-2 px-4 py-2 border-t border-border">
        <button
          onClick={onAccept}
          disabled={isPending}
          className="px-3 py-1 text-sm bg-accent text-bg-base rounded
                     hover:opacity-90 disabled:opacity-50"
        >
          Accept
        </button>
        <button
          onClick={onReject}
          disabled={isPending}
          className="px-3 py-1 text-sm border border-border rounded
                     hover:bg-bg-surface disabled:opacity-50"
        >
          Reject
        </button>
      </div>
    </div>
  );
}
