import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { uuid } from "@/lib/utils";
import * as api from "@/lib/api";
import type { TransferMode, TransferTableNameCase, TransferTask } from "@/lib/api";

export interface SaveTransferTaskInput {
  id?: string;
  name: string;
  sourceConnectionId: string;
  sourceDatabase: string;
  sourceSchema: string;
  targetConnectionId: string;
  targetDatabase: string;
  targetSchema: string;
  tables: string[];
  createTable: boolean;
  mode: TransferMode;
  targetTableNameCase: TransferTableNameCase;
  batchSize: number;
  skipCount?: boolean;
}

function nowIso() {
  return new Date().toISOString();
}

function sortTasks(items: TransferTask[]) {
  return [...items].sort((a, b) => {
    const orderDiff = (a.orderIndex ?? 0) - (b.orderIndex ?? 0);
    if (orderDiff !== 0) return orderDiff;
    return a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: "base" });
  });
}

function maxOrderIndex(values: Array<{ orderIndex?: number }>) {
  return values.reduce((max, item) => Math.max(max, item.orderIndex ?? -1), -1);
}

export const useTransferTaskStore = defineStore("transferTask", () => {
  const tasks = ref<TransferTask[]>([]);
  const isLoaded = ref(false);
  let initFromStoragePromise: Promise<void> | null = null;

  const version = ref(0);
  function bumpVersion() {
    version.value++;
  }

  async function initFromStorage() {
    if (isLoaded.value) return;
    if (!initFromStoragePromise) {
      initFromStoragePromise = (async () => {
        tasks.value = sortTasks(await api.loadTransferTasks());
        isLoaded.value = true;
        bumpVersion();
      })().finally(() => {
        initFromStoragePromise = null;
      });
    }
    await initFromStoragePromise;
  }

  function getTask(id: string) {
    return tasks.value.find((task) => task.id === id);
  }

  async function saveTask(input: SaveTransferTaskInput) {
    const timestamp = nowIso();
    const existing = input.id ? getTask(input.id) : undefined;
    const task: TransferTask = existing
      ? {
          ...existing,
          name: input.name,
          sourceConnectionId: input.sourceConnectionId,
          sourceDatabase: input.sourceDatabase,
          sourceSchema: input.sourceSchema,
          targetConnectionId: input.targetConnectionId,
          targetDatabase: input.targetDatabase,
          targetSchema: input.targetSchema,
          tables: input.tables,
          createTable: input.createTable,
          mode: input.mode,
          targetTableNameCase: input.targetTableNameCase,
          batchSize: input.batchSize,
          skipCount: input.skipCount ?? false,
          updatedAt: timestamp,
        }
      : {
          id: uuid(),
          name: input.name,
          sourceConnectionId: input.sourceConnectionId,
          sourceDatabase: input.sourceDatabase,
          sourceSchema: input.sourceSchema,
          targetConnectionId: input.targetConnectionId,
          targetDatabase: input.targetDatabase,
          targetSchema: input.targetSchema,
          tables: input.tables,
          createTable: input.createTable,
          mode: input.mode,
          targetTableNameCase: input.targetTableNameCase,
          batchSize: input.batchSize,
          skipCount: input.skipCount ?? false,
          orderIndex: maxOrderIndex(tasks.value) + 1,
          createdAt: timestamp,
          updatedAt: timestamp,
        };
    const saved = await api.saveTransferTask(task);
    tasks.value = sortTasks([...tasks.value.filter((item) => item.id !== saved.id), saved]);
    bumpVersion();
    return saved;
  }

  async function recordTaskRun(id: string) {
    const existing = getTask(id);
    if (!existing) return;
    const saved = await api.saveTransferTask({
      ...existing,
      lastRunAt: nowIso(),
      updatedAt: nowIso(),
    });
    tasks.value = sortTasks(tasks.value.map((task) => (task.id === id ? saved : task)));
    bumpVersion();
    return saved;
  }

  async function deleteTask(id: string) {
    await api.deleteTransferTask(id);
    tasks.value = tasks.value.filter((task) => task.id !== id);
    bumpVersion();
  }

  const allTasks = computed(() => sortTasks(tasks.value));

  return {
    tasks,
    isLoaded,
    version,
    initFromStorage,
    getTask,
    saveTask,
    recordTaskRun,
    deleteTask,
    allTasks,
  };
});
