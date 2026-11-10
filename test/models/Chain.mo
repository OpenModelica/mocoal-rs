package T "Three gains in a row: the test model of mocoal"
  connector In = input Real;
  connector Out = output Real;

  block Gain
    parameter Real k = 1;
    In u annotation(Placement(transformation(extent={{-140,-20},{-100,20}})));
    Out y annotation(Placement(transformation(extent={{100,-10},{120,10}})));
  equation
    y = k*u;
  end Gain;

  model Chain
    Gain a(k = 2);
    Gain b;
    Gain c "the last one";
  equation
    a.u = time;
    connect(a.y, b.u);
    connect(b.y, c.u);
  end Chain;
end T;
