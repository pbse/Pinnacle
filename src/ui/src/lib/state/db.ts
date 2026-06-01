import Dexie, { type Table } from 'dexie';

export interface DocumentRecord {
  id?: number;
  path: string;
  name: string;
  timestamp: number;
  tags: string[];
  fullText?: string;
  layoutJson?: string;
  summary?: string;
  lastPage?: number;
  totalPages?: number;
  collectionId?: number;
  thumbnail?: string;
  hash?: string;
}

export interface CollectionRecord {
  id?: number;
  name: string;
  timestamp: number;
}

export interface ChatRecord {
  id?: number;
  docPath: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: number;
}

export interface BookmarkRecord {
  id?: number;
  docPath: string;
  pageNumber: number;
  label: string;
  timestamp: number;
}

export interface ActionRecord {
  id?: number;
  type: string;
  description: string;
  timestamp: number;
  resultPath?: string;
}

export interface WatcherRecord {
  id?: number;
  query: string;
  active: boolean;
  timestamp: number;
}

export interface EntityRecord {
  id?: number;
  name: string;
  type: 'org' | 'person' | 'date' | 'location';
  docPaths: string[]; // List of documents where this entity appears
}

export interface VersionRecord {
  id?: number;
  docPath: string;
  timestamp: number;
  label: string;
  data: Uint8Array; // Original file content at that point
}

export interface NoteRecord {
  id?: number;
  content: string;
  timestamp: number;
  tags: string[];
  citations?: { docPath: string, pageNumber: number, text: string }[];
}

export class AppDatabase extends Dexie {
  documents!: Table<DocumentRecord>;
  chats!: Table<ChatRecord>;
  bookmarks!: Table<BookmarkRecord>;
  collections!: Table<CollectionRecord>;
  actions!: Table<ActionRecord>;
  watchers!: Table<WatcherRecord>;
  entities!: Table<EntityRecord>;
  versions!: Table<VersionRecord>;
  notes!: Table<NoteRecord>;

  constructor() {
    super('PinnacleDB');
    this.version(9).stores({
      documents: '++id, path, name, timestamp, *tags, collectionId, hash',
      chats: '++id, docPath, timestamp',
      bookmarks: '++id, docPath, timestamp',
      collections: '++id, name, timestamp',
      actions: '++id, type, timestamp',
      watchers: '++id, query, timestamp',
      entities: '++id, name, type, *docPaths',
      versions: '++id, docPath, timestamp',
      notes: '++id, timestamp, *tags'
    });
  }
}

export const db = new AppDatabase();
