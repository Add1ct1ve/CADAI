import { nanoid } from 'nanoid';
import type {
  MeasureToolId,
  MeasurementId,
  MeasurePoint,
  Measurement,
  MassProperties,
  ObjectId,
} from '$lib/types/cad';

// ─── State ──────────────────────────────────────
let activeMeasureTool = $state<MeasureToolId | null>(null);
let measurements = $state<Measurement[]>([]);
let pendingPoints = $state<MeasurePoint[]>([]);
let massPropertiesFor = $state<ObjectId | null>(null);
let massProperties = $state<MassProperties | null>(null);

// ─── Store ──────────────────────────────────────
export function getMeasureStore() {
  return {
    get activeMeasureTool() {
      return activeMeasureTool;
    },
    get measurements() {
      return measurements;
    },
    get pendingPoints() {
      return pendingPoints;
    },
    get massPropertiesFor() {
      return massPropertiesFor;
    },
    get massProperties() {
      return massProperties;
    },

    setMeasureTool(tool: MeasureToolId | null) {
      activeMeasureTool = tool;
      pendingPoints = [];
    },

    addPendingPoint(point: MeasurePoint) {
      pendingPoints = [...pendingPoints, point];

      // Auto-create measurement when enough points are collected
      switch (activeMeasureTool) {
        case 'measure-distance':
          if (pendingPoints.length >= 2) {
            const p1 = pendingPoints[0];
            const p2 = pendingPoints[1];
            const dx = p2.worldPos[0] - p1.worldPos[0];
            const dy = p2.worldPos[1] - p1.worldPos[1];
            const dz = p2.worldPos[2] - p1.worldPos[2];
            const distance = Math.sqrt(dx * dx + dy * dy + dz * dz);
            const m: Measurement = {
              type: 'distance',
              id: nanoid(10),
              point1: p1,
              point2: p2,
              distance,
            };
            measurements = [...measurements, m];
            pendingPoints = [];
          }
          break;

        case 'measure-angle':
          if (pendingPoints.length >= 3) {
            const vertex = pendingPoints[0];
            const arm1 = pendingPoints[1];
            const arm2 = pendingPoints[2];
            // Compute angle at vertex
            const v1 = [
              arm1.worldPos[0] - vertex.worldPos[0],
              arm1.worldPos[1] - vertex.worldPos[1],
              arm1.worldPos[2] - vertex.worldPos[2],
            ];
            const v2 = [
              arm2.worldPos[0] - vertex.worldPos[0],
              arm2.worldPos[1] - vertex.worldPos[1],
              arm2.worldPos[2] - vertex.worldPos[2],
            ];
            const dot = v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2];
            const mag1 = Math.sqrt(v1[0] * v1[0] + v1[1] * v1[1] + v1[2] * v1[2]);
            const mag2 = Math.sqrt(v2[0] * v2[0] + v2[1] * v2[1] + v2[2] * v2[2]);
            const cosAngle = mag1 > 0 && mag2 > 0 ? dot / (mag1 * mag2) : 0;
            const angleDeg = Math.acos(Math.max(-1, Math.min(1, cosAngle))) * (180 / Math.PI);
            const m: Measurement = {
              type: 'angle',
              id: nanoid(10),
              vertex,
              arm1,
              arm2,
              angleDegrees: angleDeg,
            };
            measurements = [...measurements, m];
            pendingPoints = [];
          }
          break;

        // radius and bbox are single-click, handled externally
        default:
          break;
      }
    },

    addMeasurement(m: Measurement) {
      measurements = [...measurements, m];
    },

    removeMeasurement(id: MeasurementId) {
      measurements = measurements.filter((m) => m.id !== id);
    },

    clearAll() {
      measurements = [];
      pendingPoints = [];
      massProperties = null;
      massPropertiesFor = null;
    },

    clearPending() {
      pendingPoints = [];
    },

    setMassProperties(objectId: ObjectId | null, props: MassProperties | null) {
      massPropertiesFor = objectId;
      massProperties = props;
    },
  };
}
