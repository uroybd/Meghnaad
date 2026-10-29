// Chart.js with only the parts the boards use, so the rest of the library is left out of the build. This file is
// imported only by `import('./chartjs')`: it is a chunk of its own, fetched when a board first draws a chart.

import {
  ArcElement,
  BarController,
  BarElement,
  CategoryScale,
  Chart,
  DoughnutController,
  Filler,
  Legend,
  LineController,
  LineElement,
  LinearScale,
  PieController,
  PointElement,
  PolarAreaController,
  RadarController,
  RadialLinearScale,
  Tooltip,
} from 'chart.js';
import { MatrixController, MatrixElement } from 'chartjs-chart-matrix';

Chart.register(
  BarController,
  LineController,
  PieController,
  DoughnutController,
  RadarController,
  PolarAreaController,
  MatrixController,
  CategoryScale,
  LinearScale,
  RadialLinearScale,
  BarElement,
  LineElement,
  PointElement,
  ArcElement,
  MatrixElement,
  Filler,
  Legend,
  Tooltip,
);

export { Chart };
