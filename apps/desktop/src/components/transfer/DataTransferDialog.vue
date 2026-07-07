<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { uuid } from "@/lib/utils";
import { useI18n } from "vue-i18n";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import SearchableSelect from "@/components/ui/searchable-select/SearchableSelect.vue";
import DangerConfirmDialog from "@/components/editor/DangerConfirmDialog.vue";
import { useConnectionStore } from "@/stores/connectionStore";
import { useTransferTaskStore } from "@/stores/transferTaskStore";
import DatabaseIcon from "@/components/icons/DatabaseIcon.vue";
import * as api from "@/lib/api";
import type { TransferMode, TransferTableNameCase, TransferTask } from "@/lib/api";
import type { DatabaseType } from "@/types/database";
import { isSchemaAware, supportsTransfer } from "@/lib/databaseCapabilities";
import { databaseOptionsForConnection } from "@/composables/useDatabaseOptions";
import { useExportTracker } from "@/composables/useExportTracker";
import { useToast } from "@/composables/useToast";
import { ArrowRightLeft, ArrowLeftRight, Loader2, Square, CheckSquare, Play, Pencil, Trash2, Bookmark } from "@lucide/vue";

const { t } = useI18n();
const { toast } = useToast();
const { startDataTransferTask } = useExportTracker();
const open = defineModel<boolean>("open", { default: false });

const props = defineProps<{
  prefillConnectionId?: string;
  prefillDatabase?: string;
}>();

const store = useConnectionStore();
const taskStore = useTransferTaskStore();

const activeTab = ref<"new" | "tasks">("new");
const editingTaskId = ref("");
const taskName = ref("");
const showSaveTaskDialog = ref(false);
const isSavingTask = ref(false);
const pendingExecuteTask = ref<TransferTask | null>(null);
const showExecuteConfirm = ref(false);
const loadingTask = ref(false);
const pendingTableSelection = ref<string[] | null>(null);

const sqlConnections = computed(() => store.connections.filter((c) => supportsTransfer(c.db_type)));

// Source state
const sourceConnectionId = ref("");
const sourceDatabase = ref("");
const sourceDatabases = ref<string[]>([]);
const sourceSchemas = ref<string[]>([]);
const sourceSchema = ref("");
const sourceTables = ref<string[]>([]);
const selectedTables = ref<Set<string>>(new Set());
const tableSearch = ref("");
const loadingTables = ref(false);

// Target state
const targetConnectionId = ref("");
const targetDatabase = ref("");
const targetDatabases = ref<string[]>([]);
const targetSchemas = ref<string[]>([]);
const targetSchema = ref("");

// Options
const createTable = ref(true);
const transferMode = ref<TransferMode>("append");
const targetTableNameCase = ref<TransferTableNameCase>("preserve");
const batchSize = ref(5000);
const skipCount = ref(false);
const isSubmitting = ref(false);

const filteredTables = computed(() => {
  const q = tableSearch.value.toLowerCase();
  return q ? sourceTables.value.filter((table) => table.toLowerCase().includes(q)) : sourceTables.value;
});

const allSelected = computed(() => filteredTables.value.length > 0 && filteredTables.value.every((table) => selectedTables.value.has(table)));

const canStart = computed(() => sourceConnectionId.value && sourceDatabase.value && targetConnectionId.value && targetDatabase.value && selectedTables.value.size > 0 && sourceConnectionId.value + sourceDatabase.value !== targetConnectionId.value + targetDatabase.value);

const canSaveTask = computed(() => canStart.value && taskName.value.trim().length > 0);

function connectionType(id: string): DatabaseType | undefined {
  return store.connections.find((c) => c.id === id)?.db_type;
}

function isMongoConnection(id: string): boolean {
  return connectionType(id) === "mongodb";
}

function getConnectionName(id: string) {
  return store.connections.find((c) => c.id === id)?.name ?? id;
}

function modeLabel(mode: TransferMode) {
  if (mode === "overwrite") return t("transfer.modeOverwrite");
  if (mode === "upsert") return t("transfer.modeUpsert");
  return t("transfer.modeAppend");
}

function buildTaskSummary(task: TransferTask) {
  return [
    `${t("transfer.taskName")}: ${task.name}`,
    `${t("transfer.source")}: ${getConnectionName(task.sourceConnectionId)} / ${task.sourceDatabase}`,
    `${t("transfer.target")}: ${getConnectionName(task.targetConnectionId)} / ${task.targetDatabase}`,
    `${t("transfer.tables")}: ${task.tables.length}`,
    `${t("transfer.transferMode")}: ${modeLabel(task.mode)}`,
    `${t("transfer.createTable")}: ${task.createTable ? t("transfer.yes") : t("transfer.no")}`,
    `${t("transfer.batchSize")}: ${task.batchSize}`,
    `${t("transfer.skipCount")}: ${task.skipCount ? t("transfer.yes") : t("transfer.no")}`,
  ].join("\n");
}

function currentTaskInput() {
  return {
    id: editingTaskId.value || undefined,
    name: taskName.value.trim(),
    sourceConnectionId: sourceConnectionId.value,
    sourceDatabase: sourceDatabase.value,
    sourceSchema: sourceSchema.value || sourceDatabase.value,
    targetConnectionId: targetConnectionId.value,
    targetDatabase: targetDatabase.value,
    targetSchema: targetSchema.value || targetDatabase.value,
    tables: [...selectedTables.value],
    createTable: createTable.value,
    mode: transferMode.value,
    targetTableNameCase: targetTableNameCase.value,
    batchSize: batchSize.value,
    skipCount: skipCount.value,
  };
}

function buildTransferRequestFromForm(transferId = uuid()): api.TransferRequest {
  const effectiveSourceSchema = sourceSchema.value || sourceDatabase.value;
  const effectiveTargetSchema = targetSchema.value || targetDatabase.value;
  return {
    transferId,
    sourceConnectionId: sourceConnectionId.value,
    sourceDatabase: sourceDatabase.value,
    sourceSchema: effectiveSourceSchema,
    targetConnectionId: targetConnectionId.value,
    targetDatabase: targetDatabase.value,
    targetSchema: effectiveTargetSchema,
    tables: [...selectedTables.value],
    createTable: createTable.value,
    mode: transferMode.value,
    targetTableNameCase: targetTableNameCase.value,
    batchSize: batchSize.value,
    skipCount: skipCount.value,
  };
}

function buildTransferRequestFromTask(task: TransferTask, transferId = uuid()): api.TransferRequest {
  return {
    transferId,
    sourceConnectionId: task.sourceConnectionId,
    sourceDatabase: task.sourceDatabase,
    sourceSchema: task.sourceSchema,
    targetConnectionId: task.targetConnectionId,
    targetDatabase: task.targetDatabase,
    targetSchema: task.targetSchema,
    tables: [...task.tables],
    createTable: task.createTable,
    mode: task.mode,
    targetTableNameCase: task.targetTableNameCase,
    batchSize: task.batchSize,
    skipCount: task.skipCount,
  };
}

function toggleSelectAll() {
  if (allSelected.value) {
    filteredTables.value.forEach((table) => selectedTables.value.delete(table));
  } else {
    filteredTables.value.forEach((table) => selectedTables.value.add(table));
  }
}

function toggleTable(table: string) {
  if (selectedTables.value.has(table)) {
    selectedTables.value.delete(table);
  } else {
    selectedTables.value.add(table);
  }
}

async function loadDatabases(connectionId: string, target: "source" | "target", preferredDatabase = "") {
  if (!connectionId) return;
  try {
    await store.ensureConnected(connectionId);
    const rawNames = isMongoConnection(connectionId) ? await api.mongoListDatabases(connectionId) : (await api.listDatabases(connectionId)).map((d) => d.name);
    const names = databaseOptionsForConnection(rawNames, store.getConfig(connectionId));
    if (target === "source") {
      sourceDatabases.value = names;
      sourceDatabase.value = preferredDatabase && names.includes(preferredDatabase) ? preferredDatabase : names.length === 1 ? names[0] : "";
    } else {
      targetDatabases.value = names;
      targetDatabase.value = preferredDatabase && names.includes(preferredDatabase) ? preferredDatabase : names.length === 1 ? names[0] : "";
    }
  } catch (err: any) {
    toast(err?.message || String(err), 5000);
    if (target === "source") sourceDatabases.value = [];
    else targetDatabases.value = [];
  }
}

async function loadSchemas(connectionId: string, database: string, side: "source" | "target", preferredSchema = "") {
  if (!connectionId || !database) return;
  if (isMongoConnection(connectionId)) {
    if (side === "source") {
      sourceSchemas.value = [];
      sourceSchema.value = database;
    } else {
      targetSchemas.value = [];
      targetSchema.value = database;
    }
    return;
  }
  try {
    const schemas = await api.listSchemas(connectionId, database);
    const selected = preferredSchema && schemas.includes(preferredSchema) ? preferredSchema : schemas.includes("public") ? "public" : (schemas[0] ?? "");
    if (side === "source") {
      sourceSchemas.value = schemas;
      sourceSchema.value = selected;
    } else {
      targetSchemas.value = schemas;
      targetSchema.value = selected;
    }
  } catch {
    if (side === "source") {
      sourceSchemas.value = [];
      sourceSchema.value = "";
    } else {
      targetSchemas.value = [];
      targetSchema.value = "";
    }
  }
}

async function loadTables() {
  if (!sourceConnectionId.value || !sourceDatabase.value) {
    sourceTables.value = [];
    return;
  }
  loadingTables.value = true;
  try {
    if (isMongoConnection(sourceConnectionId.value)) {
      sourceTables.value = (await api.mongoListCollections(sourceConnectionId.value, sourceDatabase.value)).map((c) => c.name);
    } else {
      const config = store.getConfig(sourceConnectionId.value);
      const needsSchema = isSchemaAware(config?.db_type);
      const schema = needsSchema && sourceSchema.value ? sourceSchema.value : sourceDatabase.value;
      const tables = await api.listTables(sourceConnectionId.value, sourceDatabase.value, schema);
      sourceTables.value = tables.filter((table) => table.table_type === "TABLE" || table.table_type === "BASE TABLE").map((table) => table.name);
    }
    const preferred = pendingTableSelection.value;
    if (preferred) {
      selectedTables.value = new Set(preferred.filter((table) => sourceTables.value.includes(table)));
      pendingTableSelection.value = null;
    } else if (selectedTables.value.size === 0) {
      selectedTables.value = new Set(sourceTables.value);
    }
  } catch {
    sourceTables.value = [];
  } finally {
    loadingTables.value = false;
  }
}

const skipSourceWatch = ref(false);
const skipTargetWatch = ref(false);

watch(sourceConnectionId, (id) => {
  if (skipSourceWatch.value) {
    skipSourceWatch.value = false;
    return;
  }
  sourceDatabase.value = "";
  sourceTables.value = [];
  selectedTables.value.clear();
  void loadDatabases(id, "source");
});

watch(sourceDatabase, async (db) => {
  if (!db || loadingTask.value) return;
  const config = store.getConfig(sourceConnectionId.value);
  if (isSchemaAware(config?.db_type)) {
    await loadSchemas(sourceConnectionId.value, db, "source");
  } else {
    sourceSchema.value = db;
  }
});

watch(sourceSchema, () => {
  if (loadingTask.value) return;
  void loadTables();
});

watch(targetConnectionId, (id) => {
  if (skipTargetWatch.value) {
    skipTargetWatch.value = false;
    return;
  }
  targetDatabase.value = "";
  targetSchemas.value = [];
  targetSchema.value = "";
  void loadDatabases(id, "target");
});

watch(targetDatabase, async (db) => {
  if (!db || loadingTask.value) return;
  const config = store.getConfig(targetConnectionId.value);
  if (isSchemaAware(config?.db_type)) {
    await loadSchemas(targetConnectionId.value, db, "target");
  } else {
    targetSchema.value = db;
  }
});

watch(
  open,
  async (val) => {
    if (val) {
      await taskStore.initFromStorage();
      resetState();
      activeTab.value = "new";
      if (props.prefillConnectionId) {
        skipSourceWatch.value = true;
        sourceConnectionId.value = props.prefillConnectionId;
        await nextTick();
        skipSourceWatch.value = false;
        await loadDatabases(props.prefillConnectionId, "source");
        if (props.prefillDatabase) {
          sourceDatabase.value = props.prefillDatabase;
        }
      }
    }
  },
  { immediate: true },
);

function resetState() {
  activeTab.value = "new";
  editingTaskId.value = "";
  taskName.value = "";
  sourceConnectionId.value = "";
  sourceDatabase.value = "";
  sourceDatabases.value = [];
  sourceSchemas.value = [];
  sourceSchema.value = "";
  sourceTables.value = [];
  selectedTables.value.clear();
  tableSearch.value = "";
  targetConnectionId.value = "";
  targetDatabase.value = "";
  targetDatabases.value = [];
  targetSchemas.value = [];
  targetSchema.value = "";
  createTable.value = true;
  transferMode.value = "append";
  targetTableNameCase.value = "preserve";
  batchSize.value = 5000;
  skipCount.value = false;
  isSubmitting.value = false;
  pendingTableSelection.value = null;
  skipSourceWatch.value = false;
  skipTargetWatch.value = false;
}

async function loadTaskIntoForm(task: TransferTask) {
  loadingTask.value = true;
  editingTaskId.value = task.id;
  taskName.value = task.name;
  createTable.value = task.createTable;
  transferMode.value = task.mode;
  targetTableNameCase.value = task.targetTableNameCase;
  batchSize.value = task.batchSize;
  skipCount.value = task.skipCount ?? false;
  pendingTableSelection.value = [...task.tables];
  selectedTables.value = new Set(task.tables);

  skipSourceWatch.value = true;
  sourceConnectionId.value = task.sourceConnectionId;
  await nextTick();
  skipSourceWatch.value = false;
  await loadDatabases(task.sourceConnectionId, "source", task.sourceDatabase);
  sourceDatabase.value = task.sourceDatabase;
  const sourceConfig = store.getConfig(task.sourceConnectionId);
  if (isSchemaAware(sourceConfig?.db_type)) {
    await loadSchemas(task.sourceConnectionId, task.sourceDatabase, "source", task.sourceSchema);
  } else {
    sourceSchema.value = task.sourceSchema;
  }
  await loadTables();

  skipTargetWatch.value = true;
  targetConnectionId.value = task.targetConnectionId;
  await nextTick();
  skipTargetWatch.value = false;
  await loadDatabases(task.targetConnectionId, "target", task.targetDatabase);
  targetDatabase.value = task.targetDatabase;
  const targetConfig = store.getConfig(task.targetConnectionId);
  if (isSchemaAware(targetConfig?.db_type)) {
    await loadSchemas(task.targetConnectionId, task.targetDatabase, "target", task.targetSchema);
  } else {
    targetSchema.value = task.targetSchema;
  }

  loadingTask.value = false;
  activeTab.value = "new";
}

function openSaveTaskDialog() {
  if (!canStart.value) return;
  if (!taskName.value.trim()) {
    const source = getConnectionName(sourceConnectionId.value);
    const target = getConnectionName(targetConnectionId.value);
    taskName.value = `${source} → ${target}`;
  }
  showSaveTaskDialog.value = true;
}

async function saveCurrentTask() {
  if (!canSaveTask.value || isSavingTask.value) return;
  isSavingTask.value = true;
  try {
    const saved = await taskStore.saveTask(currentTaskInput());
    editingTaskId.value = saved.id;
    taskName.value = saved.name;
    toast(t("transfer.taskSaved"), 2000);
    showSaveTaskDialog.value = false;
    if (!editingTaskId.value) {
      taskName.value = "";
    }
  } catch (error: any) {
    toast(error?.message || String(error), 4000);
  } finally {
    isSavingTask.value = false;
  }
}

function runTransfer(request: api.TransferRequest, label: string, shouldRefreshTargetTree: boolean) {
  const targetConnection = request.targetConnectionId;
  const targetDatabaseName = request.targetDatabase;
  const effectiveTargetSchema = request.targetSchema;
  startDataTransferTask(request, label, {
    formatOverlapError: (tables) => t("transfer.targetTableBusy", { tables: tables.join(", ") }),
    onDone: async () => {
      if (shouldRefreshTargetTree) {
        await store.refreshObjectListTreeNode(targetConnection, targetDatabaseName, effectiveTargetSchema);
      }
    },
  });
}

async function startTransfer() {
  if (!canStart.value || isSubmitting.value) return;
  isSubmitting.value = true;
  const request = buildTransferRequestFromForm();
  runTransfer(request, `${request.sourceDatabase} → ${request.targetDatabase}`, createTable.value);
  open.value = false;
  resetState();
  isSubmitting.value = false;
}

function requestExecuteTask(task: TransferTask) {
  pendingExecuteTask.value = task;
  showExecuteConfirm.value = true;
}

async function confirmExecuteTask() {
  const task = pendingExecuteTask.value;
  if (!task) return;
  const request = buildTransferRequestFromTask(task);
  runTransfer(request, task.name, task.createTable);
  await taskStore.recordTaskRun(task.id);
  pendingExecuteTask.value = null;
  showExecuteConfirm.value = false;
  open.value = false;
  resetState();
}

async function editTask(task: TransferTask) {
  await loadTaskIntoForm(task);
}

async function removeTask(task: TransferTask) {
  try {
    await taskStore.deleteTask(task.id);
    if (editingTaskId.value === task.id) {
      editingTaskId.value = "";
      taskName.value = "";
    }
    toast(t("transfer.taskDeleted"), 2000);
  } catch (error: any) {
    toast(error?.message || String(error), 4000);
  }
}

function formatLastRunAt(value?: string | null) {
  if (!value) return t("transfer.taskNeverRun");
  try {
    return new Date(value).toLocaleString();
  } catch {
    return value;
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-[780px] max-h-[80vh] flex flex-col overflow-hidden" @interact-outside.prevent>
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <ArrowRightLeft class="w-4 h-4" />
          {{ t("transfer.title") }}
        </DialogTitle>
      </DialogHeader>

      <div class="flex gap-2 border-b pb-2">
        <Button variant="ghost" size="sm" class="h-7 text-xs" :class="activeTab === 'new' ? 'bg-muted' : ''" @click="activeTab = 'new'">
          {{ t("transfer.newTask") }}
        </Button>
        <Button variant="ghost" size="sm" class="h-7 text-xs" :class="activeTab === 'tasks' ? 'bg-muted' : ''" @click="activeTab = 'tasks'">
          {{ t("transfer.savedTasks") }}
          <span v-if="taskStore.allTasks.length" class="ml-1 text-muted-foreground">({{ taskStore.allTasks.length }})</span>
        </Button>
      </div>

      <div v-if="activeTab === 'tasks'" class="flex-1 min-h-0 overflow-auto py-3">
        <div v-if="taskStore.allTasks.length === 0" class="text-xs text-muted-foreground py-8 text-center">
          {{ t("transfer.noSavedTasks") }}
        </div>
        <div v-else class="space-y-2">
          <div v-for="task in taskStore.allTasks" :key="task.id" class="border rounded-md p-3 space-y-2">
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <div class="text-sm font-medium truncate">{{ task.name }}</div>
                <div class="text-xs text-muted-foreground mt-1">
                  {{ getConnectionName(task.sourceConnectionId) }} / {{ task.sourceDatabase }}
                  →
                  {{ getConnectionName(task.targetConnectionId) }} / {{ task.targetDatabase }}
                </div>
                <div class="text-xs text-muted-foreground mt-1">
                  {{ t("transfer.taskMeta", { tables: task.tables.length, mode: modeLabel(task.mode) }) }}
                </div>
                <div class="text-xs text-muted-foreground mt-1">
                  {{ t("transfer.taskLastRun", { time: formatLastRunAt(task.lastRunAt) }) }}
                </div>
              </div>
              <div class="flex items-center gap-1 shrink-0">
                <Button variant="outline" size="icon-xs" :title="t('transfer.executeTask')" @click="requestExecuteTask(task)">
                  <Play class="w-3.5 h-3.5" />
                </Button>
                <Button variant="outline" size="icon-xs" :title="t('transfer.editConfig')" @click="editTask(task)">
                  <Pencil class="w-3.5 h-3.5" />
                </Button>
                <Button variant="outline" size="icon-xs" :title="t('transfer.deleteTask')" @click="removeTask(task)">
                  <Trash2 class="w-3.5 h-3.5" />
                </Button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="flex-1 min-h-0 overflow-auto">
        <div class="grid gap-4 py-3">
          <div class="grid grid-cols-[1fr_auto_1fr] gap-4 items-start">
            <div class="space-y-3">
              <div class="text-sm font-medium text-blue-500">
                {{ t("transfer.source") }}
              </div>

              <div class="space-y-1.5">
                <Label class="text-xs">{{ t("transfer.sourceConnection") }}</Label>
                <SearchableSelect
                  v-model="sourceConnectionId"
                  :options="sqlConnections.map((c) => c.id)"
                  :placeholder="t('transfer.selectConnection')"
                  :search-placeholder="t('transfer.searchConnection')"
                  :empty-text="t('common.noResults')"
                  :display-name="getConnectionName"
                  trigger-variant="outline"
                  trigger-class="h-8 w-full justify-between text-xs"
                  content-class="w-[var(--reka-popover-trigger-width)]"
                >
                  <template #option-label="{ option, label }">
                    <div class="flex items-center gap-1.5">
                      <DatabaseIcon :db-type="sqlConnections.find((c) => c.id === option)?.db_type ?? 'mysql'" class="w-3.5 h-3.5" />
                      {{ label }}
                    </div>
                  </template>
                </SearchableSelect>
              </div>

              <div class="space-y-1.5">
                <Label class="text-xs">{{ t("transfer.sourceDatabase") }}</Label>
                <SearchableSelect
                  v-model="sourceDatabase"
                  :options="sourceDatabases"
                  :placeholder="t('transfer.selectDatabase')"
                  :search-placeholder="t('transfer.searchDatabase')"
                  :empty-text="t('common.noResults')"
                  :disabled="!sourceDatabases.length"
                  trigger-variant="outline"
                  trigger-class="h-8 w-full justify-between text-xs"
                  content-class="w-[var(--reka-popover-trigger-width)]"
                />
              </div>

              <div v-if="sourceSchemas.length" class="space-y-1.5">
                <Label class="text-xs">{{ t("transfer.sourceSchema") }}</Label>
                <SearchableSelect
                  v-model="sourceSchema"
                  :options="sourceSchemas"
                  :placeholder="t('transfer.selectSchema')"
                  :search-placeholder="t('transfer.searchSchema')"
                  :empty-text="t('common.noResults')"
                  trigger-variant="outline"
                  trigger-class="h-8 w-full justify-between text-xs"
                  content-class="w-[var(--reka-popover-trigger-width)]"
                />
              </div>
            </div>

            <div class="flex items-center pt-8">
              <ArrowLeftRight class="w-5 h-5 text-muted-foreground" />
            </div>

            <div class="space-y-3">
              <div class="text-sm font-medium text-green-500">
                {{ t("transfer.target") }}
              </div>

              <div class="space-y-1.5">
                <Label class="text-xs">{{ t("transfer.targetConnection") }}</Label>
                <SearchableSelect
                  v-model="targetConnectionId"
                  :options="sqlConnections.map((c) => c.id)"
                  :placeholder="t('transfer.selectConnection')"
                  :search-placeholder="t('transfer.searchConnection')"
                  :empty-text="t('common.noResults')"
                  :display-name="getConnectionName"
                  trigger-variant="outline"
                  trigger-class="h-8 w-full justify-between text-xs"
                  content-class="w-[var(--reka-popover-trigger-width)]"
                >
                  <template #option-label="{ option, label }">
                    <div class="flex items-center gap-1.5">
                      <DatabaseIcon :db-type="sqlConnections.find((c) => c.id === option)?.db_type ?? 'mysql'" class="w-3.5 h-3.5" />
                      {{ label }}
                    </div>
                  </template>
                </SearchableSelect>
              </div>

              <div class="space-y-1.5">
                <Label class="text-xs">{{ t("transfer.targetDatabase") }}</Label>
                <SearchableSelect
                  v-model="targetDatabase"
                  :options="targetDatabases"
                  :placeholder="t('transfer.selectDatabase')"
                  :search-placeholder="t('transfer.searchDatabase')"
                  :empty-text="t('common.noResults')"
                  :disabled="!targetDatabases.length"
                  trigger-variant="outline"
                  trigger-class="h-8 w-full justify-between text-xs"
                  content-class="w-[var(--reka-popover-trigger-width)]"
                />
              </div>

              <div v-if="targetSchemas.length" class="space-y-1.5">
                <Label class="text-xs">{{ t("transfer.targetSchema") }}</Label>
                <SearchableSelect
                  v-model="targetSchema"
                  :options="targetSchemas"
                  :placeholder="t('transfer.selectSchema')"
                  :search-placeholder="t('transfer.searchSchema')"
                  :empty-text="t('common.noResults')"
                  trigger-variant="outline"
                  trigger-class="h-8 w-full justify-between text-xs"
                  content-class="w-[var(--reka-popover-trigger-width)]"
                />
              </div>
            </div>
          </div>

          <div class="space-y-2">
            <div class="flex items-center justify-between">
              <div class="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                {{ t("transfer.tables") }}
                <span v-if="sourceTables.length" class="text-muted-foreground/60">({{ selectedTables.size }}/{{ sourceTables.length }})</span>
              </div>
              <Button v-if="sourceTables.length" variant="ghost" size="sm" class="h-6 text-xs px-2" @click="toggleSelectAll">
                {{ allSelected ? t("transfer.deselectAll") : t("transfer.selectAll") }}
              </Button>
            </div>

            <Input v-if="sourceTables.length > 5" v-model="tableSearch" :placeholder="t('transfer.searchTables')" class="h-7 text-xs" />

            <div v-if="loadingTables" class="flex items-center gap-2 text-xs text-muted-foreground py-4 justify-center">
              <Loader2 class="w-3.5 h-3.5 animate-spin" />
              {{ t("common.loading") }}
            </div>
            <div v-else-if="!sourceConnectionId || !sourceDatabase" class="text-xs text-muted-foreground py-4 text-center">
              {{ t("transfer.selectSourceFirst") }}
            </div>
            <div v-else-if="sourceTables.length === 0" class="text-xs text-muted-foreground py-4 text-center">
              {{ t("transfer.noTables") }}
            </div>
            <div v-else class="border rounded-md max-h-[200px] overflow-y-auto">
              <div v-for="table in filteredTables" :key="table" class="flex items-center gap-2 px-2.5 py-1.5 hover:bg-muted/50 cursor-pointer text-xs" @click="toggleTable(table)">
                <CheckSquare v-if="selectedTables.has(table)" class="w-3.5 h-3.5 text-primary shrink-0" />
                <Square v-else class="w-3.5 h-3.5 text-muted-foreground/40 shrink-0" />
                <span class="truncate">{{ table }}</span>
              </div>
            </div>
          </div>

          <div class="space-y-2.5">
            <div class="flex items-center gap-2 cursor-pointer text-xs" @click="createTable = !createTable">
              <CheckSquare v-if="createTable" class="w-3.5 h-3.5 text-primary shrink-0" />
              <Square v-else class="w-3.5 h-3.5 text-muted-foreground/40 shrink-0" />
              {{ t("transfer.createTable") }}
            </div>
            <div class="flex items-center gap-3">
              <Label class="text-xs shrink-0">{{ t("transfer.transferMode") }}</Label>
              <Select v-model="transferMode">
                <SelectTrigger class="h-7 text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="append">{{ t("transfer.modeAppend") }}</SelectItem>
                  <SelectItem value="overwrite">{{ t("transfer.modeOverwrite") }}</SelectItem>
                  <SelectItem value="upsert">{{ t("transfer.modeUpsert") }}</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div class="flex items-center gap-3">
              <Label class="text-xs shrink-0">{{ t("transfer.targetTableNameCase") }}</Label>
              <Select v-model="targetTableNameCase">
                <SelectTrigger class="h-7 text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="preserve">{{ t("transfer.tableNameCasePreserve") }}</SelectItem>
                  <SelectItem value="lower">{{ t("transfer.tableNameCaseLower") }}</SelectItem>
                  <SelectItem value="upper">{{ t("transfer.tableNameCaseUpper") }}</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div class="flex items-center gap-3">
              <Label class="text-xs shrink-0">{{ t("transfer.batchSize") }}</Label>
              <Input v-model.number="batchSize" type="number" min="100" max="20000" step="100" class="h-7 text-xs w-24" />
            </div>
            <div class="flex items-center gap-2 cursor-pointer text-xs" @click="skipCount = !skipCount">
              <CheckSquare v-if="skipCount" class="w-3.5 h-3.5 text-primary shrink-0" />
              <Square v-else class="w-3.5 h-3.5 text-muted-foreground/40 shrink-0" />
              {{ t("transfer.skipCount") }}
            </div>
          </div>
        </div>
      </div>

      <DialogFooter v-if="activeTab === 'new'">
        <Button variant="outline" size="sm" @click="open = false">
          {{ t("transfer.cancel") }}
        </Button>
        <Button variant="outline" size="sm" :disabled="!canStart" @click="openSaveTaskDialog">
          <Bookmark class="w-3.5 h-3.5 mr-1.5" />
          {{ editingTaskId ? t("transfer.updateTask") : t("transfer.saveAsTask") }}
        </Button>
        <Button size="sm" :disabled="!canStart || isSubmitting" @click="startTransfer">
          <Loader2 v-if="isSubmitting" class="w-3.5 h-3.5 mr-1.5 animate-spin" />
          <ArrowRightLeft v-else class="w-3.5 h-3.5 mr-1.5" />
          {{ t("transfer.start") }}
        </Button>
      </DialogFooter>
      <DialogFooter v-else>
        <Button variant="outline" size="sm" @click="open = false">
          {{ t("transfer.cancel") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <Dialog v-model:open="showSaveTaskDialog">
    <DialogContent class="sm:max-w-[420px]" @interact-outside.prevent>
      <DialogHeader>
        <DialogTitle>{{ editingTaskId ? t("transfer.updateTask") : t("transfer.saveAsTask") }}</DialogTitle>
      </DialogHeader>
      <div class="space-y-2 py-2">
        <Label class="text-xs">{{ t("transfer.taskName") }}</Label>
        <Input v-model="taskName" :placeholder="t('transfer.taskNamePlaceholder')" class="h-8 text-xs" />
      </div>
      <DialogFooter>
        <Button variant="outline" size="sm" @click="showSaveTaskDialog = false">{{ t("transfer.cancel") }}</Button>
        <Button size="sm" :disabled="!canSaveTask || isSavingTask" @click="saveCurrentTask">
          <Loader2 v-if="isSavingTask" class="w-3.5 h-3.5 mr-1.5 animate-spin" />
          {{ t("common.save") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <DangerConfirmDialog
    v-model:open="showExecuteConfirm"
    :title="t('transfer.executeConfirmTitle')"
    :message="t('transfer.executeConfirmMessage')"
    :details-text="pendingExecuteTask ? buildTaskSummary(pendingExecuteTask) : ''"
    :confirm-label="t('transfer.executeTask')"
    @confirm="confirmExecuteTask"
  />
</template>
