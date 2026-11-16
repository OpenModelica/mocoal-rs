package Features "The rules of the layout: own connectors, inheritance, unconnected parts, Diagram frames"
  model Filter "Own connectors without a place: they get one in the icon and in the diagram"
    Parts.RealInput u annotation(Placement(transformation(origin={-100,0}, extent={{-10,-10},{10,10}}), iconTransformation(origin={-110,0}, extent={{-10,-10},{10,10}})));
    Parts.RealOutput y annotation(Placement(transformation(origin={100,0}, extent={{-10,-10},{10,10}}), iconTransformation(origin={110,0}, extent={{-10,-10},{10,10}})));
    Parts.FirstOrder stage1(T = 0.1) annotation(Placement(transformation(origin={-50,0}, extent={{-10,-10},{10,10}})));
    Parts.FirstOrder stage2(T = 0.2) annotation(Placement(transformation(origin={0,0}, extent={{-10,-10},{10,10}})));
    Parts.Gain amplify(k = 3) annotation(Placement(transformation(origin={50,0}, extent={{-10,-10},{10,10}})));
  equation
    connect(u, stage1.u) annotation(Line(points={{-100,0},{-62,0}}, color={0,0,127}));
    connect(stage1.y, stage2.u) annotation(Line(points={{-39,0},{-12,0}}, color={0,0,127}));
    connect(stage2.y, amplify.u) annotation(Line(points={{11,0},{38,0}}, color={0,0,127}));
    connect(amplify.y, y) annotation(Line(points={{61,0},{100,0}}, color={0,0,127}));
    annotation(Diagram(coordinateSystem(extent={{-140,-100},{140,100}})));
  end Filter;

  model PlacedConnectors "Own connectors with a place in the icon: it becomes the iconTransformation"
    Parts.RealInput u annotation(Placement(transformation(origin={-50,0}, extent={{-20,-20},{20,20}}), iconTransformation(extent = {{-120, 40}, {-80, 80}})));
    Parts.RealOutput y annotation(Placement(transformation(origin={60,0}, extent={{-10,-10},{10,10}}), iconTransformation(extent = {{100, -70}, {120, -50}})));
    Parts.Integrator integrator annotation(Placement(transformation(origin={10,0}, extent={{-10,-10},{10,10}})));
  equation
    connect(u, integrator.u) annotation(Line(points={{-50,0},{-2,0}}, color={0,0,127}));
    connect(integrator.y, y) annotation(Line(points={{21,0},{60,0}}, color={0,0,127}));
  end PlacedConnectors;

  model Base "Laid out in the same run as Derived"
    Parts.Constant source(k = 2) annotation(Placement(transformation(origin={-30,0}, extent={{-10,-10},{10,10}})));
    Parts.Gain gain annotation(Placement(transformation(origin={20,0}, extent={{-10,-10},{10,10}})));
  equation
    connect(source.y, gain.u) annotation(Line(points={{-19,0},{8,0}}, color={0,0,127}));
  end Base;

  model Derived "The inherited components sit where Base has them"
    extends Base;
    Parts.Integrator integrator annotation(Placement(transformation(origin={70,0}, extent={{-10,-10},{10,10}})));
    Parts.Gain amplifier annotation(Placement(transformation(origin={120,0}, extent={{-10,-10},{10,10}})));
  equation
    connect(gain.y, integrator.u) annotation(Line(points={{31,0},{58,0}}, color={0,0,127}));
    connect(integrator.y, amplifier.u) annotation(Line(points={{81,0},{108,0}}, color={0,0,127}));
    annotation(Diagram(coordinateSystem(extent={{-100,-100},{160,100}})));
  end Derived;

  model Unconnected "Connected parts in a row, unconnected ones in a grid"
    Parts.Sine sine annotation(Placement(transformation(origin={-50,50}, extent={{-10,-10},{10,10}})));
    Parts.Gain gain annotation(Placement(transformation(origin={0,50}, extent={{-10,-10},{10,10}})));
    Parts.Constant spare1 annotation(Placement(transformation(origin={-50,-20}, extent={{-10,-10},{10,10}})));
    Parts.Constant spare2 annotation(Placement(transformation(origin={0,-20}, extent={{-10,-10},{10,10}})));
    Parts.Gain spare3 annotation(Placement(transformation(origin={50,-20}, extent={{-10,-10},{10,10}})));
    Parts.Resistor spare4 annotation(Placement(transformation(origin={-50,-60}, extent={{-10,-10},{10,10}})));
    Parts.Inertia spare5 annotation(Placement(transformation(origin={0,-60}, extent={{-10,-10},{10,10}})));
  equation
    connect(sine.y, gain.u) annotation(Line(points={{-39,50},{-12,50}}, color={0,0,127}));
  end Unconnected;

  model Framed "An old layout with a frame and its title around two components, and a junction marker"
    Parts.Sine sine annotation(Placement(transformation(origin={-70,0}, extent={{-10,-10},{10,10}})));
    Parts.Gain gain1 annotation(Placement(transformation(origin={-20,0}, extent={{-10,-10},{10,10}})));
    Parts.Gain gain2 annotation(Placement(transformation(origin={30,0}, extent={{-10,-10},{10,10}})));
    Parts.Integrator integrator annotation(Placement(transformation(origin={80,20}, extent={{-10,-10},{10,10}})));
    Parts.Gain sensor annotation(Placement(transformation(origin={80,-20}, extent={{-10,-10},{10,10}})));
  equation
    connect(sine.y, gain1.u) annotation(Line(points={{-59,0},{-32,0}}, color={0,0,127}));
    connect(gain1.y, gain2.u) annotation(Line(points={{-9,0},{18,0}}, color={0,0,127}));
    connect(gain2.y, integrator.u) annotation(Line(points={{41,0},{60,0},{60,20},{68,20}}, color={0,0,127}));
    connect(gain2.y, sensor.u) annotation(Line(points={{41,0},{60,0},{60,-20},{68,-20}}, color={0,0,127}));
    annotation(Diagram(graphics = {Rectangle(extent={{-42,-22},{52,22}}, lineColor = {255, 0, 0}, pattern = LinePattern.Dash),
    Text(extent={{-30,32},{40,24}}, textString = "gains")}, coordinateSystem(extent={{-100,-100},{120,100}})));
  end Framed;

  model Interface "Only placed connectors and no connections: left as it is"
    Parts.RealInput a annotation(Placement(transformation(extent = {{-120, -10}, {-100, 10}})));
    Parts.RealOutput b annotation(Placement(transformation(extent = {{100, -10}, {120, 10}})));
  end Interface;
end Features;
