import {
  CanvasTexture,
  Color,
  CylinderGeometry,
  Euler,
  Mesh,
  MeshBasicMaterial,
  Object3D,
  OrthographicCamera,
  Quaternion,
  Raycaster,
  Sprite,
  SpriteMaterial,
  SRGBColorSpace,
  Vector2,
  Vector3,
  Vector4,
  type Camera,
  type WebGLRenderer,
} from 'three';

interface LabelOptions {
  labelX?: string;
  labelY?: string;
  labelZ?: string;
  font?: string;
  color?: string;
  radius?: number;
}

export interface ConfigurableViewHelperOptions {
  dimension?: number;
  axisScale?: number;
}

/**
 * Local copy of Three.js ViewHelper with configurable viewport size and gizmo scale.
 */
export class ConfigurableViewHelper extends Object3D {
  readonly isViewHelper = true;
  animating = false;
  center = new Vector3();

  private readonly camera: Camera;
  private readonly domElement: HTMLElement;
  private readonly dimension: number;
  private readonly axisScale: number;
  private readonly interactiveObjects: Object3D[] = [];
  private readonly raycaster = new Raycaster();
  private readonly mouse = new Vector2();
  private readonly dummy = new Object3D();
  private readonly point = new Vector3();
  private readonly targetPosition = new Vector3();
  private readonly targetQuaternion = new Quaternion();
  private readonly q1 = new Quaternion();
  private readonly q2 = new Quaternion();
  private readonly viewport = new Vector4();
  private readonly orthoCamera = new OrthographicCamera(-2, 2, 2, -2, 0, 4);
  private readonly options: LabelOptions = {};
  private readonly colorPosX = new Color('#ff4466');
  private readonly colorPosY = new Color('#88ff44');
  private readonly colorPosZ = new Color('#4488ff');
  private readonly colorNeg = new Color('#000000');

  private radius = 0;
  private readonly turnRate = 2 * Math.PI;

  private readonly geometry: CylinderGeometry;
  private readonly xAxis: Mesh;
  private readonly yAxis: Mesh;
  private readonly zAxis: Mesh;

  private readonly posXAxisHelper: Sprite;
  private readonly posYAxisHelper: Sprite;
  private readonly posZAxisHelper: Sprite;
  private readonly negXAxisHelper: Sprite;
  private readonly negYAxisHelper: Sprite;
  private readonly negZAxisHelper: Sprite;

  constructor(camera: Camera, domElement: HTMLElement, options: ConfigurableViewHelperOptions = {}) {
    super();

    this.camera = camera;
    this.domElement = domElement;
    this.dimension = options.dimension ?? 176;
    this.axisScale = options.axisScale ?? 1.25;

    this.orthoCamera.position.set(0, 0, 2);

    this.geometry = new CylinderGeometry(0.04, 0.04, 0.8, 5)
      .rotateZ(-Math.PI / 2)
      .translate(0.4, 0, 0);

    this.xAxis = new Mesh(this.geometry, this.getAxisMaterial(this.colorPosX));
    this.yAxis = new Mesh(this.geometry, this.getAxisMaterial(this.colorPosY));
    this.zAxis = new Mesh(this.geometry, this.getAxisMaterial(this.colorPosZ));

    this.yAxis.rotation.z = Math.PI / 2;
    this.zAxis.rotation.y = -Math.PI / 2;

    this.add(this.xAxis);
    this.add(this.zAxis);
    this.add(this.yAxis);

    this.posXAxisHelper = new Sprite(this.getSpriteMaterial(this.colorPosX));
    this.posYAxisHelper = new Sprite(this.getSpriteMaterial(this.colorPosY));
    this.posZAxisHelper = new Sprite(this.getSpriteMaterial(this.colorPosZ));
    this.negXAxisHelper = new Sprite(this.getSpriteMaterial(this.colorNeg));
    this.negYAxisHelper = new Sprite(this.getSpriteMaterial(this.colorNeg));
    this.negZAxisHelper = new Sprite(this.getSpriteMaterial(this.colorNeg));

    this.posXAxisHelper.position.x = 1;
    this.posYAxisHelper.position.y = 1;
    this.posZAxisHelper.position.z = 1;
    this.negXAxisHelper.position.x = -1;
    this.negYAxisHelper.position.y = -1;
    this.negZAxisHelper.position.z = -1;

    this.negXAxisHelper.material.opacity = 0.2;
    this.negYAxisHelper.material.opacity = 0.2;
    this.negZAxisHelper.material.opacity = 0.2;

    this.posXAxisHelper.userData.type = 'posX';
    this.posYAxisHelper.userData.type = 'posY';
    this.posZAxisHelper.userData.type = 'posZ';
    this.negXAxisHelper.userData.type = 'negX';
    this.negYAxisHelper.userData.type = 'negY';
    this.negZAxisHelper.userData.type = 'negZ';

    this.add(this.posXAxisHelper);
    this.add(this.posYAxisHelper);
    this.add(this.posZAxisHelper);
    this.add(this.negXAxisHelper);
    this.add(this.negYAxisHelper);
    this.add(this.negZAxisHelper);

    this.interactiveObjects.push(this.posXAxisHelper);
    this.interactiveObjects.push(this.posYAxisHelper);
    this.interactiveObjects.push(this.posZAxisHelper);
    this.interactiveObjects.push(this.negXAxisHelper);
    this.interactiveObjects.push(this.negYAxisHelper);
    this.interactiveObjects.push(this.negZAxisHelper);

    // Uniform gizmo scale (axes + caps) for better readability and clickability.
    this.scale.setScalar(this.axisScale);
  }

  render(renderer: WebGLRenderer & { isWebGPURenderer?: boolean }): void {
    this.quaternion.copy(this.camera.quaternion).invert();
    this.updateMatrixWorld();

    this.point.set(0, 0, 1);
    this.point.applyQuaternion(this.camera.quaternion);

    const x = this.domElement.offsetWidth - this.dimension;
    const y = renderer.isWebGPURenderer ? this.domElement.offsetHeight - this.dimension : 0;

    renderer.clearDepth();
    renderer.getViewport(this.viewport);
    renderer.setViewport(x, y, this.dimension, this.dimension);
    renderer.render(this, this.orthoCamera);
    renderer.setViewport(this.viewport.x, this.viewport.y, this.viewport.z, this.viewport.w);
  }

  handleClick(event: PointerEvent): boolean {
    if (this.animating) return false;

    const rect = this.domElement.getBoundingClientRect();
    const offsetX = rect.left + (this.domElement.offsetWidth - this.dimension);
    const offsetY = rect.top + (this.domElement.offsetHeight - this.dimension);

    this.mouse.x = ((event.clientX - offsetX) / (rect.right - offsetX)) * 2 - 1;
    this.mouse.y = -((event.clientY - offsetY) / (rect.bottom - offsetY)) * 2 + 1;

    this.raycaster.setFromCamera(this.mouse, this.orthoCamera);

    const intersects = this.raycaster.intersectObjects(this.interactiveObjects, false);
    if (intersects.length === 0) return false;

    this.prepareAnimationData(intersects[0].object, this.center);
    this.animating = true;
    return true;
  }

  setLabels(labelX?: string, labelY?: string, labelZ?: string): void {
    this.options.labelX = labelX;
    this.options.labelY = labelY;
    this.options.labelZ = labelZ;
    this.updateLabels();
  }

  setLabelStyle(font?: string, color?: string, radius?: number): void {
    this.options.font = font;
    this.options.color = color;
    this.options.radius = radius;
    this.updateLabels();
  }

  update(delta: number): void {
    const step = delta * this.turnRate;

    this.q1.rotateTowards(this.q2, step);
    this.camera.position
      .set(0, 0, 1)
      .applyQuaternion(this.q1)
      .multiplyScalar(this.radius)
      .add(this.center);

    this.camera.quaternion.rotateTowards(this.targetQuaternion, step);

    if (this.q1.angleTo(this.q2) === 0) {
      this.animating = false;
    }
  }

  dispose(): void {
    this.geometry.dispose();

    (this.xAxis.material as MeshBasicMaterial).dispose();
    (this.yAxis.material as MeshBasicMaterial).dispose();
    (this.zAxis.material as MeshBasicMaterial).dispose();

    this.posXAxisHelper.material.map?.dispose();
    this.posYAxisHelper.material.map?.dispose();
    this.posZAxisHelper.material.map?.dispose();
    this.negXAxisHelper.material.map?.dispose();
    this.negYAxisHelper.material.map?.dispose();
    this.negZAxisHelper.material.map?.dispose();

    this.posXAxisHelper.material.dispose();
    this.posYAxisHelper.material.dispose();
    this.posZAxisHelper.material.dispose();
    this.negXAxisHelper.material.dispose();
    this.negYAxisHelper.material.dispose();
    this.negZAxisHelper.material.dispose();
  }

  private prepareAnimationData(object: Object3D, focusPoint: Vector3): void {
    switch (object.userData.type) {
      case 'posX':
        this.targetPosition.set(1, 0, 0);
        this.targetQuaternion.setFromEuler(new Euler(0, Math.PI * 0.5, 0));
        break;
      case 'posY':
        this.targetPosition.set(0, 1, 0);
        this.targetQuaternion.setFromEuler(new Euler(-Math.PI * 0.5, 0, 0));
        break;
      case 'posZ':
        this.targetPosition.set(0, 0, 1);
        this.targetQuaternion.setFromEuler(new Euler());
        break;
      case 'negX':
        this.targetPosition.set(-1, 0, 0);
        this.targetQuaternion.setFromEuler(new Euler(0, -Math.PI * 0.5, 0));
        break;
      case 'negY':
        this.targetPosition.set(0, -1, 0);
        this.targetQuaternion.setFromEuler(new Euler(Math.PI * 0.5, 0, 0));
        break;
      case 'negZ':
        this.targetPosition.set(0, 0, -1);
        this.targetQuaternion.setFromEuler(new Euler(0, Math.PI, 0));
        break;
      default:
        console.error('ConfigurableViewHelper: Invalid axis.');
        break;
    }

    this.radius = this.camera.position.distanceTo(focusPoint);
    this.targetPosition.multiplyScalar(this.radius).add(focusPoint);

    this.dummy.position.copy(focusPoint);
    this.dummy.lookAt(this.camera.position);
    this.q1.copy(this.dummy.quaternion);

    this.dummy.lookAt(this.targetPosition);
    this.q2.copy(this.dummy.quaternion);
  }

  private getAxisMaterial(color: Color): MeshBasicMaterial {
    return new MeshBasicMaterial({ color, toneMapped: false });
  }

  private useOffscreenCanvas(): boolean {
    try {
      return typeof OffscreenCanvas !== 'undefined' && (new OffscreenCanvas(1, 1).getContext('2d')) !== null;
    } catch {
      return false;
    }
  }

  private createCanvas(width: number, height: number): OffscreenCanvas | HTMLCanvasElement {
    if (this.useOffscreenCanvas()) {
      return new OffscreenCanvas(width, height);
    }

    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    return canvas;
  }

  private getSpriteMaterial(color: Color, text?: string): SpriteMaterial {
    const {
      font = '24px Arial',
      color: labelColor = '#000000',
      radius = 14 * this.axisScale,
    } = this.options;

    const canvas = this.createCanvas(64, 64);
    const context = canvas.getContext('2d');
    if (!context) {
      const fallbackTexture = new CanvasTexture(document.createElement('canvas'));
      fallbackTexture.colorSpace = SRGBColorSpace;
      return new SpriteMaterial({ map: fallbackTexture, toneMapped: false });
    }

    context.beginPath();
    context.arc(32, 32, radius, 0, 2 * Math.PI);
    context.closePath();
    context.fillStyle = color.getStyle();
    context.fill();

    if (text) {
      context.font = font;
      context.textAlign = 'center';
      context.fillStyle = labelColor;
      context.fillText(text, 32, 41);
    }

    const texture = new CanvasTexture(canvas);
    texture.colorSpace = SRGBColorSpace;

    return new SpriteMaterial({ map: texture, toneMapped: false });
  }

  private updateLabels(): void {
    this.posXAxisHelper.material.map?.dispose();
    this.posYAxisHelper.material.map?.dispose();
    this.posZAxisHelper.material.map?.dispose();

    this.posXAxisHelper.material.dispose();
    this.posYAxisHelper.material.dispose();
    this.posZAxisHelper.material.dispose();

    this.posXAxisHelper.material = this.getSpriteMaterial(this.colorPosX, this.options.labelX);
    this.posYAxisHelper.material = this.getSpriteMaterial(this.colorPosY, this.options.labelY);
    this.posZAxisHelper.material = this.getSpriteMaterial(this.colorPosZ, this.options.labelZ);
  }
}
