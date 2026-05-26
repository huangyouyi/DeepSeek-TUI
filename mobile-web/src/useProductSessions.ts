import { useCallback, useEffect, useRef, useState } from "react";
import {
  createSession,
  deleteSession,
  listMessages,
  listSessions,
  updateSessionTitle,
  type ApiContext
} from "./api";
import type { Message, SessionSummary } from "./types";

export type UseProductSessionsOptions = {
  api?: ApiContext;
};

export type UseProductSessionsResult = {
  sessions: SessionSummary[];
  activeSessionId: string | null;
  activeMessages: Message[];
  loading: boolean;
  error: string | null;
  selectSession: (id: string) => Promise<void>;
  createConversation: (title?: string) => Promise<SessionSummary>;
  deleteConversation: (id: string) => Promise<void>;
  updateConversationTitle: (id: string, title: string) => Promise<SessionSummary>;
  reloadSessions: () => Promise<void>;
  reloadMessages: (id: string) => Promise<void>;
};

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function mostRecentSession(sessions: SessionSummary[]): SessionSummary | undefined {
  return [...sessions].sort((left, right) => {
    const updatedDiff = right.updated_at_ms - left.updated_at_ms;
    if (updatedDiff !== 0) {
      return updatedDiff;
    }
    return right.created_at_ms - left.created_at_ms;
  })[0];
}

function readUrlSessionId(): string | null {
  return new URLSearchParams(window.location.search).get("session");
}

function replaceUrlSessionId(sessionId: string | null): void {
  const url = new URL(window.location.href);
  if (sessionId) {
    url.searchParams.set("session", sessionId);
  } else {
    url.searchParams.delete("session");
  }

  const nextUrl = `${url.pathname}${url.search}${url.hash}`;
  window.history.replaceState(window.history.state, "", nextUrl);
}

export function useProductSessions(options: UseProductSessionsOptions = {}): UseProductSessionsResult {
  const api = options.api;
  const requestIdRef = useRef(0);
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [activeMessages, setActiveMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const clearActiveSession = useCallback(() => {
    setActiveSessionId(null);
    setActiveMessages([]);
    replaceUrlSessionId(null);
  }, []);

  const loadMessagesForSession = useCallback(
    async (id: string): Promise<void> => {
      const requestId = requestIdRef.current + 1;
      requestIdRef.current = requestId;
      const messages = await listMessages(id, api);
      if (requestIdRef.current !== requestId) {
        return;
      }
      setActiveSessionId(id);
      setActiveMessages(messages);
      replaceUrlSessionId(id);
    },
    [api]
  );

  const selectSession = useCallback(
    async (id: string): Promise<void> => {
      setLoading(true);
      setError(null);
      try {
        await loadMessagesForSession(id);
      } catch (caughtError) {
        setError(errorMessage(caughtError));
        throw caughtError;
      } finally {
        setLoading(false);
      }
    },
    [loadMessagesForSession]
  );

  const loadSessions = useCallback(
    async (preferredSessionId?: string | null): Promise<SessionSummary[]> => {
      setLoading(true);
      setError(null);
      try {
        const loadedSessions = await listSessions(api);
        setSessions(loadedSessions);

        if (loadedSessions.length === 0) {
          clearActiveSession();
          return loadedSessions;
        }

        const preferredSession = preferredSessionId
          ? loadedSessions.find((session) => session.id === preferredSessionId)
          : undefined;
        const nextSession = preferredSession ?? mostRecentSession(loadedSessions);
        if (nextSession) {
          await loadMessagesForSession(nextSession.id);
        } else {
          clearActiveSession();
        }

        return loadedSessions;
      } catch (caughtError) {
        setError(errorMessage(caughtError));
        throw caughtError;
      } finally {
        setLoading(false);
      }
    },
    [api, clearActiveSession, loadMessagesForSession]
  );

  const reloadSessions = useCallback(async (): Promise<void> => {
    await loadSessions(activeSessionId);
  }, [activeSessionId, loadSessions]);

  const reloadMessages = useCallback(
    async (id: string): Promise<void> => {
      await selectSession(id);
    },
    [selectSession]
  );

  const createConversation = useCallback(
    async (title?: string): Promise<SessionSummary> => {
      setLoading(true);
      setError(null);
      try {
        const created = await createSession(api);
        const conversation = title ? await updateSessionTitle(created.id, title, api) : created;
        setSessions((currentSessions) => [
          conversation,
          ...currentSessions.filter((session) => session.id !== conversation.id)
        ]);
        await loadMessagesForSession(conversation.id);
        return conversation;
      } catch (caughtError) {
        setError(errorMessage(caughtError));
        throw caughtError;
      } finally {
        setLoading(false);
      }
    },
    [api, loadMessagesForSession]
  );

  const deleteConversation = useCallback(
    async (id: string): Promise<void> => {
      setLoading(true);
      setError(null);
      try {
        await deleteSession(id, api);
        const loadedSessions = await listSessions(api);
        setSessions(loadedSessions);

        if (loadedSessions.length === 0) {
          clearActiveSession();
          return;
        }

        const activeSessionStillExists = loadedSessions.some((session) => session.id === activeSessionId);
        const shouldSelectNext = activeSessionId === id || !activeSessionStillExists;
        const nextSessionId = shouldSelectNext ? mostRecentSession(loadedSessions)?.id : activeSessionId;
        if (nextSessionId) {
          await loadMessagesForSession(nextSessionId);
        } else {
          clearActiveSession();
        }
      } catch (caughtError) {
        setError(errorMessage(caughtError));
        throw caughtError;
      } finally {
        setLoading(false);
      }
    },
    [activeSessionId, api, clearActiveSession, loadMessagesForSession]
  );

  const updateConversationTitle = useCallback(
    async (id: string, title: string): Promise<SessionSummary> => {
      setLoading(true);
      setError(null);
      try {
        const updated = await updateSessionTitle(id, title, api);
        setSessions((currentSessions) =>
          currentSessions.map((session) => (session.id === id ? updated : session))
        );
        return updated;
      } catch (caughtError) {
        setError(errorMessage(caughtError));
        throw caughtError;
      } finally {
        setLoading(false);
      }
    },
    [api]
  );

  useEffect(() => {
    async function initializeSessions(): Promise<void> {
      try {
        await loadSessions(readUrlSessionId());
      } catch {
        // The hook exposes the error state for rendering.
      }
    }

    initializeSessions();

    return () => {
      requestIdRef.current += 1;
    };
  }, [loadSessions]);

  return {
    sessions,
    activeSessionId,
    activeMessages,
    loading,
    error,
    selectSession,
    createConversation,
    deleteConversation,
    updateConversationTitle,
    reloadSessions,
    reloadMessages
  };
}
