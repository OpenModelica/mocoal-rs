model Drive "Speed control of an inertia; text only, no graphical annotations"
  Modelica.Blocks.Sources.Step step(startTime = 0.1) annotation(Placement(transformation(origin={-150,20}, extent={{-10,-10},{10,10}})));
  Modelica.Blocks.Math.Feedback feedback annotation(Placement(transformation(origin={-100,0}, extent={{-10,-10},{10,10}})));
  Modelica.Blocks.Continuous.PI pi(k = 10, T = 0.1) annotation(Placement(transformation(origin={-50,0}, extent={{-10,-10},{10,10}})));
  Modelica.Mechanics.Rotational.Sources.Torque torque annotation(Placement(transformation(origin={0,0}, extent={{-10,-10},{10,10}})));
  Modelica.Mechanics.Rotational.Components.Inertia inertia(J = 1) annotation(Placement(transformation(origin={50,0}, extent={{-10,-10},{10,10}})));
  Modelica.Mechanics.Rotational.Components.Damper damper(d = 0.5) annotation(Placement(transformation(origin={100,0}, extent={{-10,-10},{10,10}})));
  Modelica.Mechanics.Rotational.Components.Fixed fixed annotation(Placement(transformation(origin={150,0}, extent={{-10,-10},{10,10}})));
  Modelica.Mechanics.Rotational.Sensors.SpeedSensor speed annotation(Placement(transformation(origin={-150,-20}, extent={{-10,-10},{10,10}})));
equation
  connect(step.y, feedback.u1) annotation(Line(points={{-139,20},{-115,20},{-115,0},{-108,0}}, color={0,0,127}));
  connect(feedback.y, pi.u) annotation(Line(points={{-91,0},{-62,0}}, color={0,0,127}));
  connect(pi.y, torque.tau) annotation(Line(points={{-39,0},{-12,0}}, color={0,0,127}));
  connect(torque.flange, inertia.flange_a) annotation(Line(points={{10,0},{40,0}}));
  connect(inertia.flange_b, damper.flange_a) annotation(Line(points={{60,0},{90,0}}));
  connect(damper.flange_b, fixed.flange) annotation(Line(points={{110,0},{150,0}}));
  connect(inertia.flange_b, speed.flange) annotation(Line(points={{60,0},{65,0},{65,-35},{-165,-35},{-165,-20},{-160,-20}}));
  connect(speed.w, feedback.u2) annotation(Line(points={{-139,-20},{-100,-20},{-100,-8}}, color={0,0,127}));
  annotation(Diagram(coordinateSystem(extent={{-200,-100},{180,100}})));
end Drive;
